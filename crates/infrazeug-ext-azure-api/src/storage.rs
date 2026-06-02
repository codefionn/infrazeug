use crate::client::AzureClient;
use crate::error::Result;
use reqwest::Method;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlobContainer {
    pub name: String,
    pub storage_account: String,
}

#[derive(Debug, Deserialize)]
struct StorageKeysResponse {
    #[serde(default)]
    keys: Vec<StorageKey>,
}

#[derive(Debug, Clone, Deserialize)]
struct StorageKey {
    #[serde(default)]
    value: Option<String>,
    #[serde(default, rename = "keyName")]
    key_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageAccountKey {
    pub storage_account: String,
    pub resource_group: String,
    pub key_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_value: Option<String>,
}

impl AzureClient {
    pub async fn blob_container_exists(
        &self,
        storage_account: &str,
        container_name: &str,
    ) -> Result<bool> {
        let url = format!(
            "https://{storage_account}.blob.core.windows.net/{container_name}?restype=container"
        );
        let (status, _) = self.storage_request(Method::HEAD, &url, &[], None).await?;
        match status {
            StatusCode::OK => Ok(true),
            StatusCode::NOT_FOUND => Ok(false),
            _ => Ok(false),
        }
    }

    pub async fn blob_container_create(
        &self,
        storage_account: &str,
        container_name: &str,
    ) -> Result<BlobContainer> {
        let url = format!(
            "https://{storage_account}.blob.core.windows.net/{container_name}?restype=container"
        );
        let (status, body) = self.storage_request(Method::PUT, &url, &[], None).await?;
        if status.is_success() {
            Ok(BlobContainer {
                name: container_name.into(),
                storage_account: storage_account.into(),
            })
        } else {
            Err(crate::error::AzureError::Api {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    pub async fn storage_account_keys(
        &self,
        resource_group: &str,
        storage_account: &str,
    ) -> Result<Vec<StorageAccountKey>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Storage/storageAccounts/{}/listKeys?api-version=2023-01-01",
            self.subscription_id(),
            resource_group,
            storage_account
        );
        let parsed: StorageKeysResponse = self.arm_post(&url, &serde_json::json!({})).await?;
        Ok(parsed
            .keys
            .into_iter()
            .filter_map(|k| {
                Some(StorageAccountKey {
                    storage_account: storage_account.into(),
                    resource_group: resource_group.into(),
                    key_name: k.key_name?,
                    key_value: k.value,
                })
            })
            .collect())
    }
}
