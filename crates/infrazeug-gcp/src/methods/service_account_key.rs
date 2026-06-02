//! Ensure a service-account key exists.

use crate::client::GcpClientSource;
use async_trait::async_trait;
use infrazeug_ext_gcp_api::iam::CreatedServiceAccountKey;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_SERVICE_ACCOUNT_KEY: &str = "gcp.ensure_service_account_key";

pub type EnsureServiceAccountKey = EnsureResource<ServiceAccountKeyResource>;

pub fn ensure_service_account_key(source: GcpClientSource) -> EnsureServiceAccountKey {
    EnsureResource::new(ServiceAccountKeyResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureServiceAccountKeyInput {
    pub service_account_email: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureServiceAccountKeyOutput {
    pub service_account_email: String,
    pub key_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key_data: Option<String>,
}

#[derive(Clone)]
pub struct ServiceAccountKeyResource {
    source: GcpClientSource,
}

impl ServiceAccountKeyResource {
    pub fn new(source: GcpClientSource) -> Self {
        Self { source }
    }
}

#[async_trait]
impl Resource for ServiceAccountKeyResource {
    type Spec = EnsureServiceAccountKeyInput;
    type State = EnsureServiceAccountKeyOutput;

    fn kind(&self) -> &'static str {
        ENSURE_SERVICE_ACCOUNT_KEY
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let keys = client
            .iam_service_account_keys(&spec.service_account_email)
            .await
            .map_err(ResourceError::provider)?;
        match keys.first() {
            Some(existing) => Ok(Some(EnsureServiceAccountKeyOutput {
                service_account_email: spec.service_account_email.clone(),
                key_name: existing.key_name.clone(),
                private_key_data: None,
            })),
            None => Ok(None),
        }
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let keys = client
            .iam_service_account_keys(&spec.service_account_email)
            .await
            .map_err(ResourceError::provider)?;
        let key: CreatedServiceAccountKey = if let Some(existing) = keys.first() {
            existing.clone()
        } else {
            client
                .iam_service_account_key_create(&spec.service_account_email)
                .await
                .map_err(ResourceError::provider)?
        };
        Ok(EnsureServiceAccountKeyOutput {
            service_account_email: key.service_account_email,
            key_name: key.key_name,
            private_key_data: key.private_key_data,
        })
    }
}
