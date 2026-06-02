//! Google Cloud Storage buckets.

use crate::client::GcpClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bucket {
    pub name: String,
    pub location: String,
}

#[derive(Debug, Deserialize)]
struct BucketResource {
    name: Option<String>,
    location: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BucketList {
    #[serde(default)]
    items: Vec<BucketResource>,
}

impl GcpClient {
    pub async fn storage_buckets(&self) -> Result<Vec<Bucket>> {
        let url = format!(
            "https://storage.googleapis.com/storage/v1/b?project={}",
            self.project_id()
        );
        let list: BucketList = self.get(&url).await?;
        Ok(list
            .items
            .into_iter()
            .filter_map(|item| {
                Some(Bucket {
                    name: item.name?,
                    location: item.location.unwrap_or_default(),
                })
            })
            .collect())
    }

    pub async fn storage_bucket_exists(&self, name: &str) -> Result<bool> {
        let url = format!("https://storage.googleapis.com/storage/v1/b/{name}");
        match self.get::<BucketResource>(&url).await {
            Ok(_) => Ok(true),
            Err(crate::error::GcpError::Api { status: 404, .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn storage_bucket_create(&self, name: &str, location: &str) -> Result<Bucket> {
        let body = serde_json::json!({
            "name": name,
            "location": location,
        });
        let url = format!(
            "https://storage.googleapis.com/storage/v1/b?project={}",
            self.project_id()
        );
        let item: BucketResource = self.post_json(&url, &body).await?;
        Ok(Bucket {
            name: item.name.unwrap_or_else(|| name.into()),
            location: item.location.unwrap_or_else(|| location.into()),
        })
    }
}
