//! Ensure Keystone EC2/S3 credentials exist for the authenticated user.

use crate::client::OpenstackClientSource;
use async_trait::async_trait;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_S3_CREDENTIALS: &str = "openstack.ensure_s3_credentials";

pub type EnsureS3Credentials = EnsureResource<S3CredentialsResource>;

pub fn ensure_s3_credentials(source: OpenstackClientSource) -> EnsureS3Credentials {
    EnsureResource::new(S3CredentialsResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureS3CredentialsInput {}

/// JSON capture payload for downstream vault writes (`/access_key_id`, `/secret_access_key`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureS3CredentialsOutput {
    pub access_key_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,
}

#[derive(Clone)]
pub struct S3CredentialsResource {
    source: OpenstackClientSource,
}

impl S3CredentialsResource {
    pub fn new(source: OpenstackClientSource) -> Self {
        Self { source }
    }
}

#[async_trait]
impl Resource for S3CredentialsResource {
    type Spec = EnsureS3CredentialsInput;
    type State = EnsureS3CredentialsOutput;

    fn kind(&self) -> &'static str {
        ENSURE_S3_CREDENTIALS
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        _spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let user_id = client.user_id().await.map_err(ResourceError::provider)?;
        let creds = client
            .list_ec2_credentials(&user_id)
            .await
            .map_err(ResourceError::provider)?;
        match creds.first() {
            Some(existing) => Ok(Some(EnsureS3CredentialsOutput {
                access_key_id: existing.access.clone(),
                secret_access_key: existing.secret.clone(),
            })),
            None => Ok(None),
        }
    }

    async fn create(&self, ctx: &ResourceCtx, _spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let user_id = client.user_id().await.map_err(ResourceError::provider)?;
        let project_id = client.project_id();

        let creds = client
            .list_ec2_credentials(&user_id)
            .await
            .map_err(ResourceError::provider)?;
        if let Some(existing) = creds.first() {
            return Ok(EnsureS3CredentialsOutput {
                access_key_id: existing.access.clone(),
                secret_access_key: existing.secret.clone(),
            });
        }

        let created = client
            .create_ec2_credential(&user_id, project_id)
            .await
            .map_err(ResourceError::provider)?;
        Ok(EnsureS3CredentialsOutput {
            access_key_id: created.access,
            secret_access_key: created.secret,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_omits_absent_secret() {
        let out = EnsureS3CredentialsOutput {
            access_key_id: "ak".into(),
            secret_access_key: None,
        };
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(
            json.pointer("/access_key_id").and_then(|v| v.as_str()),
            Some("ak")
        );
        assert!(json.pointer("/secret_access_key").is_none());
    }
}
