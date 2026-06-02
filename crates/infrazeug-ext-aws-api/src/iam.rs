//! IAM user and access-key operations (Query API).

use crate::client::{ensure_success, AwsClient};
use crate::error::{AwsError, Result};
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};

const IAM_VERSION: &str = "2010-05-08";

/// IAM access key summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessKey {
    pub user_name: String,
    pub access_key_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,
}

impl AwsClient {
    /// `GetUser` — returns `None` when the user does not exist.
    pub async fn iam_user_exists(&self, user_name: &str) -> Result<bool> {
        let params = vec![
            ("Action".into(), "GetUser".into()),
            ("Version".into(), IAM_VERSION.into()),
            ("UserName".into(), user_name.into()),
        ];
        let (status, body) = self.iam_query(&params).await?;
        if status.as_u16() == 404 || body.contains("<Code>NoSuchEntity</Code>") {
            return Ok(false);
        }
        ensure_success(status, &body)?;
        Ok(true)
    }

    /// `CreateUser`.
    pub async fn iam_user_create(&self, user_name: &str) -> Result<()> {
        let params = vec![
            ("Action".into(), "CreateUser".into()),
            ("Version".into(), IAM_VERSION.into()),
            ("UserName".into(), user_name.into()),
        ];
        let (status, body) = self.iam_query(&params).await?;
        ensure_success(status, &body)
    }

    /// `ListAccessKeys` for a user.
    pub async fn iam_access_keys(&self, user_name: &str) -> Result<Vec<AccessKey>> {
        let params = vec![
            ("Action".into(), "ListAccessKeys".into()),
            ("Version".into(), IAM_VERSION.into()),
            ("UserName".into(), user_name.into()),
        ];
        let (status, body) = self.iam_query(&params).await?;
        ensure_success(status, &body)?;
        parse_access_keys(user_name, &body, None)
    }

    /// `CreateAccessKey` — the secret is returned only at creation time.
    pub async fn iam_access_key_create(&self, user_name: &str) -> Result<AccessKey> {
        let params = vec![
            ("Action".into(), "CreateAccessKey".into()),
            ("Version".into(), IAM_VERSION.into()),
            ("UserName".into(), user_name.into()),
        ];
        let (status, body) = self.iam_query(&params).await?;
        ensure_success(status, &body)?;
        parse_access_keys(user_name, &body, None)?
            .into_iter()
            .next()
            .ok_or_else(|| AwsError::Api {
                status: status.as_u16(),
                message: "CreateAccessKey returned no key".into(),
            })
    }
}

#[derive(Debug, Deserialize)]
struct AccessKeyItem {
    #[serde(rename = "accessKeyId")]
    access_key_id: Option<String>,
    #[serde(rename = "secretAccessKey")]
    secret_access_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AccessKeyMetadataItem {
    #[serde(rename = "accessKeyId")]
    access_key_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AccessKeyMetadata {
    #[serde(default, rename = "member")]
    members: Vec<AccessKeyMetadataItem>,
}

#[derive(Debug, Deserialize)]
struct ListAccessKeysResult {
    #[serde(rename = "AccessKeyMetadata")]
    access_key_metadata: Option<AccessKeyMetadata>,
}

#[derive(Debug, Deserialize)]
struct CreateAccessKeyResult {
    #[serde(rename = "AccessKey")]
    access_key: Option<AccessKeyItem>,
}

fn parse_access_keys(
    user_name: &str,
    body: &str,
    secret: Option<String>,
) -> Result<Vec<AccessKey>> {
    if let Ok(resp) = from_str::<CreateAccessKeyResponse>(body) {
        if let Some(ak) = resp.create_access_key_result.and_then(|r| r.access_key) {
            return Ok(vec![AccessKey {
                user_name: user_name.into(),
                access_key_id: ak.access_key_id.unwrap_or_default(),
                secret_access_key: ak.secret_access_key.or(secret),
            }]);
        }
    }
    if let Ok(resp) = from_str::<ListAccessKeysResponse>(body) {
        let mut out = Vec::new();
        if let Some(meta) = resp
            .list_access_keys_result
            .and_then(|r| r.access_key_metadata)
        {
            for item in meta.members {
                if let Some(id) = item.access_key_id {
                    out.push(AccessKey {
                        user_name: user_name.into(),
                        access_key_id: id,
                        secret_access_key: None,
                    });
                }
            }
        }
        return Ok(out);
    }
    Err(AwsError::Xml(format!("unexpected IAM response: {body}")))
}

#[derive(Debug, Deserialize)]
struct CreateAccessKeyResponse {
    #[serde(rename = "CreateAccessKeyResult")]
    create_access_key_result: Option<CreateAccessKeyResult>,
}

#[derive(Debug, Deserialize)]
struct ListAccessKeysResponse {
    #[serde(rename = "ListAccessKeysResult")]
    list_access_keys_result: Option<ListAccessKeysResult>,
}
