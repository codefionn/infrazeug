//! B2 application key management (`b2_list_keys`, `b2_create_key`, `b2_delete_key`).

use crate::client::BackblazeClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// An existing application key (secret is never returned by list).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ApplicationKey {
    #[serde(
        rename = "applicationKeyId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub application_key_id: Option<String>,
    #[serde(rename = "keyName", default, skip_serializing_if = "Option::is_none")]
    pub key_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    #[serde(rename = "accountId", default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(rename = "bucketIds", default, skip_serializing_if = "Option::is_none")]
    pub bucket_ids: Option<Vec<String>>,
    #[serde(
        rename = "namePrefix",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub name_prefix: Option<String>,
    #[serde(
        rename = "expirationTimestamp",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub expiration_timestamp: Option<i64>,
}

/// A newly created application key (secret returned once).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ApplicationKeyCreateResponse {
    #[serde(
        rename = "applicationKeyId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub application_key_id: Option<String>,
    #[serde(
        rename = "applicationKey",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub application_key: Option<String>,
    #[serde(rename = "keyName", default, skip_serializing_if = "Option::is_none")]
    pub key_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    #[serde(rename = "accountId", default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(rename = "bucketIds", default, skip_serializing_if = "Option::is_none")]
    pub bucket_ids: Option<Vec<String>>,
    #[serde(
        rename = "namePrefix",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub name_prefix: Option<String>,
    #[serde(
        rename = "expirationTimestamp",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub expiration_timestamp: Option<i64>,
}

/// Body for `b2_create_key`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct ApplicationKeyCreate {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "keyName")]
    pub key_name: String,
    pub capabilities: Vec<String>,
    #[serde(rename = "bucketIds", default, skip_serializing_if = "Option::is_none")]
    pub bucket_ids: Option<Vec<String>>,
    #[serde(
        rename = "namePrefix",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub name_prefix: Option<String>,
    #[serde(
        rename = "validDurationInSeconds",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub valid_duration_in_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ListKeysResponse {
    #[serde(default)]
    keys: Vec<ApplicationKey>,
    #[serde(rename = "nextApplicationKeyId", default)]
    next_application_key_id: Option<String>,
}

impl BackblazeClient {
    /// `b2_list_keys` — list application keys (paginated).
    pub async fn list_application_keys(&self) -> Result<Vec<ApplicationKey>> {
        let account_id = self.account_id().await?;
        let mut start: Option<String> = None;
        let mut out = Vec::new();
        loop {
            let mut body = serde_json::json!({
                "accountId": account_id,
                "maxKeys": 1000,
            });
            if let Some(start_id) = &start {
                body["startApplicationKeyId"] = start_id.clone().into();
            }
            let page: ListKeysResponse = self.post_json("b2_list_keys", &body).await?;
            let empty = page.keys.is_empty();
            out.extend(page.keys);
            start = page.next_application_key_id;
            if start.is_none() || empty {
                break;
            }
        }
        Ok(out)
    }

    /// Find an application key by display name.
    pub async fn application_key_by_name(&self, key_name: &str) -> Result<Option<ApplicationKey>> {
        Ok(self
            .list_application_keys()
            .await?
            .into_iter()
            .find(|k| k.key_name.as_deref() == Some(key_name)))
    }

    /// `b2_create_key` — create an application key.
    pub async fn create_application_key(
        &self,
        body: &ApplicationKeyCreate,
    ) -> Result<ApplicationKeyCreateResponse> {
        self.post_json("b2_create_key", body).await
    }

    /// `b2_delete_key` — delete an application key by id.
    pub async fn delete_application_key(&self, application_key_id: &str) -> Result<()> {
        let account_id = self.account_id().await?;
        let body = serde_json::json!({
            "accountId": account_id,
            "applicationKeyId": application_key_id,
        });
        let _: ApplicationKey = self.post_json("b2_delete_key", &body).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_key_deserializes() {
        let body = r#"{
            "applicationKeyId": "005",
            "keyName": "backup",
            "capabilities": ["listBuckets", "readFiles"]
        }"#;
        let key: ApplicationKey = serde_json::from_str(body).unwrap();
        assert_eq!(key.key_name.as_deref(), Some("backup"));
    }

    #[test]
    fn application_key_create_serializes() {
        let body = ApplicationKeyCreate {
            account_id: "acc".into(),
            key_name: "backup".into(),
            capabilities: vec!["readFiles".into()],
            bucket_ids: Some(vec!["bid".into()]),
            ..Default::default()
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains(r#""keyName":"backup""#));
        assert!(json.contains(r#""bucketIds":["bid"]"#));
    }
}
