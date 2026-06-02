//! Ensure an IAM user with an access key exists.

use crate::client::AwsClientSource;
use async_trait::async_trait;
use infrazeug_ext_aws_api::iam::AccessKey;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_IAM_ACCESS_KEY: &str = "aws.ensure_iam_access_key";

pub type EnsureIamAccessKey = EnsureResource<IamAccessKeyResource>;

pub fn ensure_iam_access_key(source: AwsClientSource) -> EnsureIamAccessKey {
    EnsureResource::new(IamAccessKeyResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureIamAccessKeyInput {
    pub user_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureIamAccessKeyOutput {
    pub user_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,
}

#[derive(Clone)]
pub struct IamAccessKeyResource {
    source: AwsClientSource,
}

impl IamAccessKeyResource {
    pub fn new(source: AwsClientSource) -> Self {
        Self { source }
    }
}

fn to_output(key: AccessKey) -> EnsureIamAccessKeyOutput {
    EnsureIamAccessKeyOutput {
        user_name: key.user_name,
        access_key_id: Some(key.access_key_id),
        secret_access_key: key.secret_access_key,
    }
}

#[async_trait]
impl Resource for IamAccessKeyResource {
    type Spec = EnsureIamAccessKeyInput;
    type State = EnsureIamAccessKeyOutput;

    fn kind(&self) -> &'static str {
        ENSURE_IAM_ACCESS_KEY
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        if !client
            .iam_user_exists(&spec.user_name)
            .await
            .map_err(ResourceError::provider)?
        {
            return Ok(None);
        }
        let keys = client
            .iam_access_keys(&spec.user_name)
            .await
            .map_err(ResourceError::provider)?;
        match keys.first() {
            Some(existing) => Ok(Some(EnsureIamAccessKeyOutput {
                user_name: spec.user_name.clone(),
                access_key_id: Some(existing.access_key_id.clone()),
                secret_access_key: None,
            })),
            None => Ok(None),
        }
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        if !client
            .iam_user_exists(&spec.user_name)
            .await
            .map_err(ResourceError::provider)?
        {
            client
                .iam_user_create(&spec.user_name)
                .await
                .map_err(ResourceError::provider)?;
        }
        let keys = client
            .iam_access_keys(&spec.user_name)
            .await
            .map_err(ResourceError::provider)?;
        let key = if let Some(existing) = keys.first() {
            AccessKey {
                user_name: spec.user_name.clone(),
                access_key_id: existing.access_key_id.clone(),
                secret_access_key: None,
            }
        } else {
            client
                .iam_access_key_create(&spec.user_name)
                .await
                .map_err(ResourceError::provider)?
        };
        Ok(to_output(key))
    }
}
