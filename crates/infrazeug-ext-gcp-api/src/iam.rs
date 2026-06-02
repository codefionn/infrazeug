//! IAM service-account keys.

use crate::client::GcpClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreatedServiceAccountKey {
    pub service_account_email: String,
    pub key_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key_data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KeyList {
    #[serde(default)]
    keys: Vec<KeyResource>,
}

#[derive(Debug, Deserialize)]
struct KeyResource {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KeyCreateResponse {
    name: Option<String>,
    #[serde(default, rename = "privateKeyData")]
    private_key_data: Option<String>,
}

impl GcpClient {
    pub async fn iam_service_account_keys(
        &self,
        service_account_email: &str,
    ) -> Result<Vec<CreatedServiceAccountKey>> {
        let email = urlencoding::encode(service_account_email);
        let url = format!(
            "https://iam.googleapis.com/v1/projects/{}/serviceAccounts/{}:keys",
            self.project_id(),
            email
        );
        let list: KeyList = self.get(&url).await?;
        Ok(list
            .keys
            .into_iter()
            .filter_map(|k| {
                Some(CreatedServiceAccountKey {
                    service_account_email: service_account_email.into(),
                    key_name: k.name?,
                    private_key_data: None,
                })
            })
            .collect())
    }

    pub async fn iam_service_account_key_create(
        &self,
        service_account_email: &str,
    ) -> Result<CreatedServiceAccountKey> {
        let email = urlencoding::encode(service_account_email);
        let url = format!(
            "https://iam.googleapis.com/v1/projects/{}/serviceAccounts/{}:keys",
            self.project_id(),
            email
        );
        let body = serde_json::json!({
            "privateKeyType": "TYPE_GOOGLE_CREDENTIALS_FILE",
            "keyAlgorithm": "KEY_ALG_RSA_2048"
        });
        let item: KeyCreateResponse = self.post_json(&url, &body).await?;
        Ok(CreatedServiceAccountKey {
            service_account_email: service_account_email.into(),
            key_name: item.name.unwrap_or_default(),
            private_key_data: item.private_key_data,
        })
    }
}
