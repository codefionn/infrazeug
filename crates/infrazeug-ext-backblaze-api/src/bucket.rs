//! B2 bucket management (`b2_list_buckets`, `b2_create_bucket`, `b2_update_bucket`).

use crate::client::BackblazeClient;
use crate::error::{BackblazeError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A B2 object-storage bucket.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Bucket {
    #[serde(rename = "accountId", default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(rename = "bucketId", default, skip_serializing_if = "Option::is_none")]
    pub bucket_id: Option<String>,
    #[serde(
        rename = "bucketName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bucket_name: Option<String>,
    #[serde(
        rename = "bucketType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bucket_type: Option<String>,
    #[serde(
        rename = "bucketInfo",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bucket_info: Option<Value>,
    #[serde(
        rename = "lifecycleRules",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub lifecycle_rules: Option<Vec<Value>>,
    #[serde(rename = "corsRules", default, skip_serializing_if = "Option::is_none")]
    pub cors_rules: Option<Vec<Value>>,
}

/// Body for `b2_create_bucket`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct BucketCreate {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "bucketName")]
    pub bucket_name: String,
    #[serde(rename = "bucketType")]
    pub bucket_type: String,
    #[serde(
        rename = "bucketInfo",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bucket_info: Option<Value>,
    #[serde(rename = "corsRules", default, skip_serializing_if = "Option::is_none")]
    pub cors_rules: Option<Vec<Value>>,
    #[serde(
        rename = "lifecycleRules",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub lifecycle_rules: Option<Vec<Value>>,
}

/// Body for `b2_update_bucket`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct BucketUpdate {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "bucketId")]
    pub bucket_id: String,
    #[serde(
        rename = "bucketType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bucket_type: Option<String>,
    #[serde(
        rename = "bucketInfo",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub bucket_info: Option<Value>,
    #[serde(rename = "corsRules", default, skip_serializing_if = "Option::is_none")]
    pub cors_rules: Option<Vec<Value>>,
    #[serde(
        rename = "lifecycleRules",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub lifecycle_rules: Option<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
struct ListBucketsResponse {
    #[serde(default)]
    buckets: Vec<Bucket>,
}

impl BackblazeClient {
    /// `b2_list_buckets` — list buckets, optionally filtered to one name.
    pub async fn list_buckets(&self, bucket_name: Option<&str>) -> Result<Vec<Bucket>> {
        let account_id = self.account_id().await?;
        let mut body = serde_json::json!({ "accountId": account_id });
        if let Some(name) = bucket_name {
            body["bucketName"] = name.into();
        }
        let resp: ListBucketsResponse = self.post_json("b2_list_buckets", &body).await?;
        Ok(resp.buckets)
    }

    /// Return the bucket with `bucket_name`, or `None` when absent.
    pub async fn try_bucket(&self, bucket_name: &str) -> Result<Option<Bucket>> {
        Ok(self
            .list_buckets(Some(bucket_name))
            .await?
            .into_iter()
            .next())
    }

    /// `b2_create_bucket` — create a bucket.
    pub async fn create_bucket(&self, body: &BucketCreate) -> Result<Bucket> {
        self.post_json("b2_create_bucket", body).await
    }

    /// `b2_update_bucket` — update bucket settings.
    pub async fn update_bucket(&self, body: &BucketUpdate) -> Result<Bucket> {
        self.post_json("b2_update_bucket", body).await
    }

    /// `b2_delete_bucket` — delete a bucket by id.
    pub async fn delete_bucket(&self, bucket_id: &str) -> Result<()> {
        let account_id = self.account_id().await?;
        let body = serde_json::json!({
            "accountId": account_id,
            "bucketId": bucket_id,
        });
        let _: Bucket = self.post_json("b2_delete_bucket", &body).await?;
        Ok(())
    }

    /// Like [`try_bucket`](Self::try_bucket) but maps `duplicate_bucket_name` to `None`.
    pub async fn try_bucket_or_absent(&self, bucket_name: &str) -> Result<Option<Bucket>> {
        match self.try_bucket(bucket_name).await {
            Ok(bucket) => Ok(bucket),
            Err(BackblazeError::Api { code, .. }) if code == "duplicate_bucket_name" => Ok(None),
            Err(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_deserializes() {
        let body = r#"{
            "accountId": "acc",
            "bucketId": "bid",
            "bucketName": "logs",
            "bucketType": "allPrivate"
        }"#;
        let bucket: Bucket = serde_json::from_str(body).unwrap();
        assert_eq!(bucket.bucket_name.as_deref(), Some("logs"));
        assert_eq!(bucket.bucket_type.as_deref(), Some("allPrivate"));
    }

    #[test]
    fn bucket_create_serializes() {
        let body = BucketCreate {
            account_id: "acc".into(),
            bucket_name: "logs".into(),
            bucket_type: "allPrivate".into(),
            ..Default::default()
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains(r#""bucketName":"logs""#));
        assert!(json.contains(r#""bucketType":"allPrivate""#));
    }
}
