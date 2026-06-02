//! S3 bucket operations (REST API).

use crate::client::{api_error, AwsClient};
use crate::error::{AwsError, Result};
use quick_xml::de::from_str;
use reqwest::Method;
use reqwest::StatusCode;
use serde::Deserialize;

/// S3 bucket summary.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Bucket {
    pub name: String,
    pub region: String,
}

impl AwsClient {
    /// `ListBuckets` — buckets owned by the authenticated account.
    pub async fn s3_buckets(&self) -> Result<Vec<Bucket>> {
        let (status, body) = self.s3_request(Method::GET, None, "/", &[], None).await?;
        ensure_success(status, &body)?;
        parse_buckets(&body, self.region())
    }

    /// `HEAD` bucket — returns `Ok` when the bucket exists.
    pub async fn s3_bucket_exists(&self, name: &str) -> Result<bool> {
        let (status, body) = self
            .s3_request(Method::HEAD, Some(name), "/", &[], None)
            .await?;
        match status {
            StatusCode::OK => Ok(true),
            StatusCode::NOT_FOUND => Ok(false),
            StatusCode::FORBIDDEN => Ok(false),
            _ => Err(api_error(status, &body)),
        }
    }

    /// `PUT` bucket — create a bucket in the configured region.
    pub async fn s3_bucket_create(&self, name: &str) -> Result<Bucket> {
        let (status, body) = self
            .s3_request(Method::PUT, Some(name), "/", &[], None)
            .await?;
        if status.is_success() {
            Ok(Bucket {
                name: name.into(),
                region: self.region().into(),
            })
        } else {
            Err(api_error(status, &body))
        }
    }
}

fn ensure_success(status: StatusCode, body: &str) -> Result<()> {
    if status.is_success() {
        Ok(())
    } else {
        Err(api_error(status, body))
    }
}

#[derive(Debug, Deserialize)]
struct BucketItem {
    #[serde(rename = "Name")]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BucketsSet {
    #[serde(default, rename = "Bucket")]
    items: Vec<BucketItem>,
}

#[derive(Debug, Deserialize)]
struct ListAllMyBucketsResult {
    #[serde(rename = "Buckets")]
    buckets: Option<BucketsSet>,
}

fn parse_buckets(body: &str, default_region: &str) -> Result<Vec<Bucket>> {
    let resp: ListAllMyBucketsResult = from_str(body).map_err(|e| AwsError::Xml(e.to_string()))?;
    Ok(resp
        .buckets
        .map(|b| {
            b.items
                .into_iter()
                .filter_map(|item| {
                    Some(Bucket {
                        name: item.name?,
                        region: default_region.into(),
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_list_buckets_xml() {
        let xml = r#"<ListAllMyBucketsResult>
            <Buckets>
                <Bucket><Name>alpha</Name></Bucket>
                <Bucket><Name>beta</Name></Bucket>
            </Buckets>
        </ListAllMyBucketsResult>"#;
        let buckets = parse_buckets(xml, "eu-west-1").unwrap();
        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].name, "alpha");
        assert_eq!(buckets[0].region, "eu-west-1");
    }
}
