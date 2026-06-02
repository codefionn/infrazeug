use crate::client::AzureClientSource;
use async_trait::async_trait;
use infrazeug_ext_azure_api::storage::StorageAccountKey;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_STORAGE_KEY: &str = "azure.ensure_storage_key";

pub type EnsureStorageKey = EnsureResource<StorageKeyResource>;

pub fn ensure_storage_key(source: AzureClientSource) -> EnsureStorageKey {
    EnsureResource::new(StorageKeyResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureStorageKeyInput {
    pub resource_group: String,
    pub storage_account: String,
    #[serde(default = "default_key_name")]
    pub key_name: String,
}

fn default_key_name() -> String {
    "key1".into()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureStorageKeyOutput {
    pub storage_account: String,
    pub resource_group: String,
    pub key_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_value: Option<String>,
}

#[derive(Clone)]
pub struct StorageKeyResource {
    source: AzureClientSource,
}

impl StorageKeyResource {
    pub fn new(source: AzureClientSource) -> Self {
        Self { source }
    }
}

#[async_trait]
impl Resource for StorageKeyResource {
    type Spec = EnsureStorageKeyInput;
    type State = EnsureStorageKeyOutput;

    fn kind(&self) -> &'static str {
        ENSURE_STORAGE_KEY
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let keys = client
            .storage_account_keys(&spec.resource_group, &spec.storage_account)
            .await
            .map_err(ResourceError::provider)?;
        match keys.iter().find(|k| k.key_name == spec.key_name) {
            Some(existing) => Ok(Some(EnsureStorageKeyOutput {
                storage_account: spec.storage_account.clone(),
                resource_group: spec.resource_group.clone(),
                key_name: existing.key_name.clone(),
                key_value: None,
            })),
            None => Ok(None),
        }
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let keys = client
            .storage_account_keys(&spec.resource_group, &spec.storage_account)
            .await
            .map_err(ResourceError::provider)?;
        let key: StorageAccountKey = keys
            .into_iter()
            .find(|k| k.key_name == spec.key_name)
            .ok_or_else(|| {
                ResourceError::provider(format!(
                    "storage account {} has no key named {}",
                    spec.storage_account, spec.key_name
                ))
            })?;
        Ok(EnsureStorageKeyOutput {
            storage_account: key.storage_account,
            resource_group: key.resource_group,
            key_name: key.key_name,
            key_value: key.key_value,
        })
    }
}
