//! Ensure a B2 application key exists (create-only; secret returned once).

use crate::client::BackblazeClientSource;
use async_trait::async_trait;
use infrazeug_ext_backblaze_api::application_key::{
    ApplicationKey, ApplicationKeyCreate, ApplicationKeyCreateResponse,
};
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_APPLICATION_KEY: &str = "backblaze.ensure_application_key";

pub type EnsureApplicationKey = EnsureResource<ApplicationKeyResource>;

pub fn ensure_application_key(source: BackblazeClientSource) -> EnsureApplicationKey {
    EnsureResource::new(ApplicationKeyResource::new(source))
}

/// Desired application key. Natural key: `key_name`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureApplicationKeyInput {
    /// Display name for the key (not required to be unique).
    pub key_name: String,
    /// Capability strings (e.g. `listBuckets`, `readFiles`, `writeFiles`).
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_duration_in_seconds: Option<u64>,
}

/// Observed application key. The secret is only present immediately after create.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureApplicationKeyOutput {
    pub application_key_id: String,
    pub key_name: String,
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiration_timestamp: Option<i64>,
}

#[derive(Clone)]
pub struct ApplicationKeyResource {
    source: BackblazeClientSource,
}

impl ApplicationKeyResource {
    pub fn new(source: BackblazeClientSource) -> Self {
        Self { source }
    }
}

fn to_output_existing(key: ApplicationKey) -> EnsureApplicationKeyOutput {
    EnsureApplicationKeyOutput {
        application_key_id: key.application_key_id.unwrap_or_default(),
        key_name: key.key_name.unwrap_or_default(),
        capabilities: key.capabilities.unwrap_or_default(),
        application_key: None,
        bucket_ids: key.bucket_ids,
        name_prefix: key.name_prefix,
        expiration_timestamp: key.expiration_timestamp,
    }
}

fn to_output_created(key: ApplicationKeyCreateResponse) -> EnsureApplicationKeyOutput {
    EnsureApplicationKeyOutput {
        application_key_id: key.application_key_id.unwrap_or_default(),
        key_name: key.key_name.unwrap_or_default(),
        capabilities: key.capabilities.unwrap_or_default(),
        application_key: key.application_key,
        bucket_ids: key.bucket_ids,
        name_prefix: key.name_prefix,
        expiration_timestamp: key.expiration_timestamp,
    }
}

#[async_trait]
impl Resource for ApplicationKeyResource {
    type Spec = EnsureApplicationKeyInput;
    type State = EnsureApplicationKeyOutput;

    fn kind(&self) -> &'static str {
        ENSURE_APPLICATION_KEY
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        Ok(client
            .application_key_by_name(&spec.key_name)
            .await
            .map_err(ResourceError::provider)?
            .map(to_output_existing))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        if let Some(existing) = client
            .application_key_by_name(&spec.key_name)
            .await
            .map_err(ResourceError::provider)?
        {
            return Ok(to_output_existing(existing));
        }
        let account_id = client.account_id().await.map_err(ResourceError::provider)?;
        let created = client
            .create_application_key(&ApplicationKeyCreate {
                account_id,
                key_name: spec.key_name.clone(),
                capabilities: spec.capabilities.clone(),
                bucket_ids: spec.bucket_ids.clone(),
                name_prefix: spec.name_prefix.clone(),
                valid_duration_in_seconds: spec.valid_duration_in_seconds,
            })
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output_created(created))
    }
}
