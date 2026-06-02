//! R2 bucket management (`/accounts/{account_id}/r2/buckets`).

use crate::client::{decode_body, CloudflareClient};
use crate::error::{CloudflareError, Result};
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// An R2 object-storage bucket.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct R2Bucket {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
}

/// Body for `POST /accounts/{account_id}/r2/buckets`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct R2BucketCreate {
    pub name: String,
    #[serde(
        rename = "locationHint",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub location_hint: Option<String>,
    #[serde(
        rename = "storageClass",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub storage_class: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct R2BucketQuery {
    pub jurisdiction: Option<String>,
    pub cursor: Option<String>,
    pub per_page: Option<u32>,
    pub name_contains: Option<String>,
}

impl R2BucketQuery {
    fn as_params(&self) -> Vec<(&str, String)> {
        let mut out = Vec::new();
        if let Some(cursor) = &self.cursor {
            out.push(("cursor", cursor.clone()));
        }
        if let Some(per_page) = self.per_page {
            out.push(("per_page", per_page.to_string()));
        }
        if let Some(name_contains) = &self.name_contains {
            out.push(("name_contains", name_contains.clone()));
        }
        out
    }

    fn jurisdiction_header(&self) -> Vec<(&str, &str)> {
        self.jurisdiction
            .as_deref()
            .map(|j| vec![("cf-r2-jurisdiction", j)])
            .unwrap_or_default()
    }
}

#[derive(Debug, Deserialize)]
struct R2BucketListResult {
    #[serde(default)]
    buckets: Vec<R2Bucket>,
}

#[derive(Debug, Deserialize)]
struct R2CursorInfo {
    cursor: Option<String>,
}

impl CloudflareClient {
    /// `GET /accounts/{account_id}/r2/buckets` — list buckets (cursor-paginated).
    pub async fn r2_buckets(
        &self,
        account_id: &str,
        query: &R2BucketQuery,
    ) -> Result<Vec<R2Bucket>> {
        let path = format!("/accounts/{}/r2/buckets", self.encode_path(account_id));
        let mut cursor = query.cursor.clone();
        let per_page = query.per_page.unwrap_or(100);
        let mut out = Vec::new();

        loop {
            let mut page_query = query.clone();
            page_query.cursor = cursor.clone();
            page_query.per_page = Some(per_page);
            let params = page_query.as_params();
            let headers = page_query.jurisdiction_header();
            let resp = self
                .send_with_headers(Method::GET, &path, &params, None, &headers)
                .await?;
            let status = resp.status();
            let body = resp.text().await?;
            let (page, info): (R2BucketListResult, Option<R2CursorInfo>) =
                decode_r2_list(status, &body)?;
            let empty = page.buckets.is_empty();
            out.extend(page.buckets);
            cursor = info.and_then(|i| i.cursor);
            if cursor.is_none() || empty {
                break;
            }
        }
        Ok(out)
    }

    /// `GET /accounts/{account_id}/r2/buckets/{name}` — fetch one bucket.
    pub async fn r2_bucket(
        &self,
        account_id: &str,
        bucket_name: &str,
        jurisdiction: Option<&str>,
    ) -> Result<R2Bucket> {
        let path = format!(
            "/accounts/{}/r2/buckets/{}",
            self.encode_path(account_id),
            self.encode_path(bucket_name)
        );
        let headers = jurisdiction
            .map(|j| [("cf-r2-jurisdiction", j)])
            .unwrap_or_default();
        self.get_with_headers(&path, &[], &headers).await
    }

    /// Like [`r2_bucket`](Self::r2_bucket) but returns `None` when the bucket is absent.
    pub async fn try_r2_bucket(
        &self,
        account_id: &str,
        bucket_name: &str,
        jurisdiction: Option<&str>,
    ) -> Result<Option<R2Bucket>> {
        match self.r2_bucket(account_id, bucket_name, jurisdiction).await {
            Ok(bucket) => Ok(Some(bucket)),
            Err(CloudflareError::Api { status: 404, .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// `POST /accounts/{account_id}/r2/buckets` — create a bucket.
    pub async fn create_r2_bucket(
        &self,
        account_id: &str,
        body: &R2BucketCreate,
        jurisdiction: Option<&str>,
    ) -> Result<R2Bucket> {
        let path = format!("/accounts/{}/r2/buckets", self.encode_path(account_id));
        let headers = jurisdiction
            .map(|j| [("cf-r2-jurisdiction", j)])
            .unwrap_or_default();
        self.post_json_with_headers(&path, body, &headers).await
    }

    /// `PATCH /accounts/{account_id}/r2/buckets/{name}` — update bucket storage class.
    pub async fn patch_r2_bucket_storage_class(
        &self,
        account_id: &str,
        bucket_name: &str,
        storage_class: &str,
        jurisdiction: Option<&str>,
    ) -> Result<R2Bucket> {
        let path = format!(
            "/accounts/{}/r2/buckets/{}",
            self.encode_path(account_id),
            self.encode_path(bucket_name)
        );
        let mut headers = vec![("cf-r2-storage-class", storage_class)];
        if let Some(jurisdiction) = jurisdiction {
            headers.push(("cf-r2-jurisdiction", jurisdiction));
        }
        self.patch_with_headers(&path, &headers).await
    }

    /// `DELETE /accounts/{account_id}/r2/buckets/{name}` — delete a bucket.
    pub async fn delete_r2_bucket(
        &self,
        account_id: &str,
        bucket_name: &str,
        jurisdiction: Option<&str>,
    ) -> Result<()> {
        let path = format!(
            "/accounts/{}/r2/buckets/{}",
            self.encode_path(account_id),
            self.encode_path(bucket_name)
        );
        let headers = jurisdiction
            .map(|j| [("cf-r2-jurisdiction", j)])
            .unwrap_or_default();
        let resp = self
            .send_with_headers(Method::DELETE, &path, &[], None, &headers)
            .await?;
        let _: Option<serde_json::Value> = decode_body(resp.status(), &resp.text().await?)?.0;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct R2ListEnvelope {
    success: bool,
    #[serde(default)]
    errors: Vec<crate::types::ApiErrorEntry>,
    result: Option<R2BucketListResult>,
    result_info: Option<R2CursorInfo>,
}

fn decode_r2_list(
    status: reqwest::StatusCode,
    body: &str,
) -> Result<(R2BucketListResult, Option<R2CursorInfo>)> {
    let envelope: R2ListEnvelope = serde_json::from_str(body)?;
    if status.is_success() && envelope.success {
        Ok((
            envelope
                .result
                .unwrap_or(R2BucketListResult { buckets: vec![] }),
            envelope.result_info,
        ))
    } else {
        let codes: Vec<u64> = envelope.errors.iter().filter_map(|e| e.code).collect();
        let message = envelope
            .errors
            .iter()
            .filter_map(|e| e.message.clone())
            .collect::<Vec<_>>()
            .join("; ");
        Err(CloudflareError::Api {
            status: status.as_u16(),
            codes,
            message: if message.is_empty() {
                body.trim().to_string()
            } else {
                message
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn r2_bucket_deserializes() {
        let body = r#"{
            "name": "my-bucket",
            "creation_date": "2024-01-01T00:00:00.000Z",
            "location": "wnam",
            "storage_class": "Standard",
            "jurisdiction": "default"
        }"#;
        let bucket: R2Bucket = serde_json::from_str(body).unwrap();
        assert_eq!(bucket.name.as_deref(), Some("my-bucket"));
        assert_eq!(bucket.storage_class.as_deref(), Some("Standard"));
    }

    #[test]
    fn r2_bucket_create_serializes_camel_case() {
        let body = R2BucketCreate {
            name: "logs".into(),
            location_hint: Some("wnam".into()),
            storage_class: Some("Standard".into()),
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains(r#""locationHint":"wnam""#));
        assert!(json.contains(r#""storageClass":"Standard""#));
    }

    #[test]
    fn decode_r2_list_page() {
        let body = r#"{
            "success": true,
            "errors": [],
            "messages": [],
            "result": {
                "buckets": [{"name": "a"}]
            },
            "result_info": {"cursor": "next", "per_page": 100}
        }"#;
        let (page, info) = decode_r2_list(StatusCode::OK, body).unwrap();
        assert_eq!(page.buckets.len(), 1);
        assert_eq!(info.and_then(|i| i.cursor).as_deref(), Some("next"));
    }
}
