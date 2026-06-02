//! Ensure a Public Cloud user's Object Storage S3 IAM policy.

use crate::client::OvhClientSource;
use async_trait::async_trait;
use infrazeug_ext_ovh_api::public_cloud::{CloudProjectUser, CloudProjectUserS3Policy};
use infrazeug_ext_ovh_api::OvhClient;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_S3_USER_POLICY: &str = "ovh.ensure_s3_user_policy";

/// Tier-1 method: ensure a project user's Object Storage S3 IAM policy.
pub type EnsureS3UserPolicy = EnsureResource<S3UserPolicyResource>;

/// Construct the registrable [`EnsureS3UserPolicy`] method for a client source.
pub fn ensure_s3_user_policy(source: OvhClientSource) -> EnsureS3UserPolicy {
    EnsureResource::new(S3UserPolicyResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureS3UserPolicyInput {
    pub project_id: String,
    pub user_description: String,
    pub policy: String,
}

/// JSON capture payload for downstream consumers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureS3UserPolicyOutput {
    pub user_id: String,
    pub policy: String,
}

/// OVH Public Cloud Object Storage user policy as an acquirable resource.
#[derive(Clone)]
pub struct S3UserPolicyResource {
    source: OvhClientSource,
}

impl S3UserPolicyResource {
    pub fn new(source: OvhClientSource) -> Self {
        Self { source }
    }
}

#[async_trait]
impl Resource for S3UserPolicyResource {
    type Spec = EnsureS3UserPolicyInput;
    type State = EnsureS3UserPolicyOutput;

    fn kind(&self) -> &'static str {
        ENSURE_S3_USER_POLICY
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let Some(user) = find_user(client.as_ref(), &spec.project_id, &spec.user_description)
            .await
            .map_err(ResourceError::provider)?
        else {
            return Ok(None);
        };
        if user.status.as_deref() != Some("ok") {
            return Ok(None);
        }

        let user_id = user.id.to_string();
        let policy = client
            .cloud_project_user_s3_policy(&spec.project_id, &user_id)
            .await
            .map_err(ResourceError::provider)?;
        Ok(Some(EnsureS3UserPolicyOutput {
            user_id,
            policy: canonical_policy(&policy.policy)?,
        }))
    }

    fn diff(&self, spec: &Self::Spec, state: &Self::State) -> Drift {
        match canonical_policy(&spec.policy) {
            Ok(want) if want == state.policy => Drift::InSync,
            Ok(_) => Drift::Drifted("S3 user policy differs".into()),
            Err(e) => Drift::Drifted(e.to_string()),
        }
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        self.apply(ctx, spec).await
    }

    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        _current: Self::State,
    ) -> ResourceResult<Self::State> {
        self.apply(ctx, spec).await
    }
}

impl S3UserPolicyResource {
    async fn apply(
        &self,
        ctx: &ResourceCtx,
        spec: &EnsureS3UserPolicyInput,
    ) -> ResourceResult<EnsureS3UserPolicyOutput> {
        let client = self.source.client(ctx).await?;
        let user = find_user(client.as_ref(), &spec.project_id, &spec.user_description)
            .await
            .map_err(ResourceError::provider)?
            .ok_or_else(|| {
                ResourceError::provider(format!(
                    "OVH Public Cloud user with description {:?} not found",
                    spec.user_description
                ))
            })?;
        let user_id = user.id.to_string();
        let policy = canonical_policy(&spec.policy)?;
        client
            .cloud_project_user_s3_policy_set(
                &spec.project_id,
                &user_id,
                &CloudProjectUserS3Policy {
                    policy: policy.clone(),
                },
            )
            .await
            .map_err(ResourceError::provider)?;
        Ok(EnsureS3UserPolicyOutput { user_id, policy })
    }
}

async fn find_user(
    client: &OvhClient,
    project_id: &str,
    description: &str,
) -> infrazeug_ext_ovh_api::Result<Option<CloudProjectUser>> {
    let users = client.cloud_project_users(project_id).await?;
    Ok(users
        .into_iter()
        .find(|u| u.description.as_deref() == Some(description)))
}

fn canonical_policy(policy: &str) -> ResourceResult<String> {
    let value: serde_json::Value = serde_json::from_str(policy).map_err(ResourceError::provider)?;
    serde_json::to_string(&value).map_err(ResourceError::provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_policy_removes_formatting() {
        let policy = canonical_policy(
            r#"{
              "Statement": [
                { "Sid": "RW", "Effect": "Allow", "Action": ["s3:ListBucket"], "Resource": ["*"] }
              ]
            }"#,
        )
        .unwrap();
        assert_eq!(
            policy,
            r#"{"Statement":[{"Action":["s3:ListBucket"],"Effect":"Allow","Resource":["*"],"Sid":"RW"}]}"#
        );
    }
}
