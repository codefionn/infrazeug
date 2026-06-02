//! Ensure a Public Cloud user with S3 credentials exists.

use crate::client::OvhClientSource;
use async_trait::async_trait;
use infrazeug_ext_ovh_api::public_cloud::{CloudProjectUser, CloudProjectUserCreate};
use infrazeug_ext_ovh_api::{OvhClient, OvhError};
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const ENSURE_S3_USER: &str = "ovh.ensure_s3_user";

const DEFAULT_ROLE: &str = "objectstore_operator";

/// OVH provisions a new project user asynchronously: `POST /user` returns it in
/// `creating` status, and its S3-credential routes 404 ("Server error") until it
/// reaches `ok`. Poll the user up to this many times before giving up.
const USER_READY_ATTEMPTS: usize = 30;
/// Delay between user-readiness polls.
const USER_READY_DELAY: Duration = Duration::from_secs(2);

/// Tier-1 method: ensure a project user with at least one S3 credential.
pub type EnsureS3User = EnsureResource<S3UserResource>;

/// Construct the registrable [`EnsureS3User`] method for a client source.
pub fn ensure_s3_user(source: OvhClientSource) -> EnsureS3User {
    EnsureResource::new(S3UserResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureS3UserInput {
    pub project_id: String,
    pub description: String,
    #[serde(default)]
    pub role_names: Vec<String>,
}

/// JSON capture payload for downstream [`FileSource::capture_same_machine`] + `json_pointer`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureS3UserOutput {
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,
}

/// OVH Public Cloud user + S3 credential as an acquirable resource.
#[derive(Clone)]
pub struct S3UserResource {
    source: OvhClientSource,
}

impl S3UserResource {
    pub fn new(source: OvhClientSource) -> Self {
        Self { source }
    }
}

#[async_trait]
impl Resource for S3UserResource {
    type Spec = EnsureS3UserInput;
    type State = EnsureS3UserOutput;

    fn kind(&self) -> &'static str {
        ENSURE_S3_USER
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let Some(user) = find_user(client.as_ref(), &spec.project_id, &spec.description)
            .await
            .map_err(ResourceError::provider)?
        else {
            return Ok(None);
        };

        // A user still provisioning can't serve its credential routes yet; defer to
        // `create`, which waits for it to become ready.
        if user.status.as_deref() != Some("ok") {
            return Ok(None);
        }

        // A user with no S3 credential yet is treated as not-yet-acquired so
        // `create` issues one (it is idempotent on the user itself).
        let user_id = user.id.to_string();
        let creds = client
            .cloud_project_user_s3_credentials(&spec.project_id, &user_id)
            .await
            .map_err(ResourceError::provider)?;
        match creds.first() {
            // The secret is only ever returned at credential creation, so an
            // already-existing credential exposes the access-key id (`access`) only.
            Some(existing) => Ok(Some(EnsureS3UserOutput {
                user_id,
                access_key_id: Some(existing.access.clone()),
                secret_access_key: None,
            })),
            None => Ok(None),
        }
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let roles = if spec.role_names.is_empty() {
            vec![DEFAULT_ROLE.to_string()]
        } else {
            spec.role_names.clone()
        };

        // Reuse the user if it already exists (observe returned `None` only because
        // it lacked a credential or was still provisioning), otherwise create it.
        let user = match find_user(client.as_ref(), &spec.project_id, &spec.description)
            .await
            .map_err(ResourceError::provider)?
        {
            Some(user) => user,
            None => client
                .cloud_project_user_create(
                    &spec.project_id,
                    &CloudProjectUserCreate {
                        description: spec.description.clone(),
                        roles,
                    },
                )
                .await
                .map_err(ResourceError::provider)?,
        };
        let user_id = user.id.to_string();

        // A freshly created user is `creating`; its S3-credential routes 404 until
        // it reaches `ok`, so wait for it before issuing/listing credentials.
        wait_user_ready(client.as_ref(), &spec.project_id, &user_id)
            .await
            .map_err(ResourceError::provider)?;

        // Reuse an existing credential (access key only) or issue a new one
        // (access key + secret, the single time the secret is returned).
        let creds = client
            .cloud_project_user_s3_credentials(&spec.project_id, &user_id)
            .await
            .map_err(ResourceError::provider)?;
        let (access_key_id, secret_access_key) = if let Some(existing) = creds.first() {
            (Some(existing.access.clone()), None)
        } else {
            let cred = client
                .cloud_project_user_s3_credential_create(&spec.project_id, &user_id)
                .await
                .map_err(ResourceError::provider)?;
            (Some(cred.access), Some(cred.secret))
        };

        Ok(EnsureS3UserOutput {
            user_id,
            access_key_id,
            secret_access_key,
        })
    }
}

async fn find_user(
    client: &OvhClient,
    project_id: &str,
    description: &str,
) -> infrazeug_ext_ovh_api::Result<Option<CloudProjectUser>> {
    // The list route returns full user objects, so match on description directly
    // (the numeric id becomes the `{userId}` path segment for credential calls).
    let users = client.cloud_project_users(project_id).await?;
    Ok(users
        .into_iter()
        .find(|u| u.description.as_deref() == Some(description)))
}

/// Poll `GET /user/{userId}` until the user reports `ok`. A 404 while polling means
/// the user is still being provisioned, so it is treated as "not ready yet" rather
/// than a hard error. Falls through after [`USER_READY_ATTEMPTS`] so the subsequent
/// credential call surfaces the real error if the user never settles.
async fn wait_user_ready(
    client: &OvhClient,
    project_id: &str,
    user_id: &str,
) -> infrazeug_ext_ovh_api::Result<()> {
    for _ in 0..USER_READY_ATTEMPTS {
        match client.cloud_project_user(project_id, user_id).await {
            Ok(user) if user.status.as_deref() == Some("ok") => return Ok(()),
            Ok(_) => {}
            Err(OvhError::Api { status: 404, .. }) => {}
            Err(e) => return Err(e),
        }
        tokio::time::sleep(USER_READY_DELAY).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_omits_absent_secret() {
        // When the secret is unknown (existing credential), the capture JSON
        // must omit `/secret_access_key` so the optional vault pointer skips it
        // rather than storing a bogus value. The access key is always present.
        let out = EnsureS3UserOutput {
            user_id: "u1".into(),
            access_key_id: Some("ak".into()),
            secret_access_key: None,
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(
            json.pointer("/access_key_id").and_then(|v| v.as_str()),
            Some("ak")
        );
        assert!(json.pointer("/secret_access_key").is_none());

        // A freshly issued credential carries both.
        let out = EnsureS3UserOutput {
            user_id: "u1".into(),
            access_key_id: Some("ak".into()),
            secret_access_key: Some("sk".into()),
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(
            json.pointer("/secret_access_key").and_then(|v| v.as_str()),
            Some("sk")
        );
    }
}
