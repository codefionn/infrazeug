//! Public Cloud **Storage (S3)** — region-scoped object-storage containers
//! (`/cloud/project/{serviceName}/region/{regionName}/storage`).
//!
//! This is OVH's S3-compatible object storage (Standard / High-Performance),
//! reachable at `s3.<region>.io.cloud.ovh.net`. It is a different product from the
//! legacy project-wide Swift endpoint (`/cloud/project/{serviceName}/storage`),
//! which 404s with "This service does not exist" on projects that only have S3.
//! For the S3 API the **container name is the identity** (there is no separate
//! container id) and the bucket lives in the region carried in the request path.

use super::region_path;
use crate::client::OvhClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// S3 object-storage container (`cloud.StorageContainer` / `cloud.StorageContainerList`).
///
/// Both the list summary and the single-container detail share these fields; the
/// richer detail-only attributes (encryption, versioning, …) are ignored. Only
/// `name` is guaranteed present; the rest are optional so one struct decodes every
/// response shape.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageContainer {
    /// Container (bucket) name — the S3 identity within the region.
    pub name: String,
    /// Region the container lives in (echoes the path's `regionName`).
    #[serde(default)]
    pub region: Option<String>,
    /// Virtual-host style endpoint for the bucket, when reported.
    #[serde(default)]
    pub virtual_host: Option<String>,
    /// Owning project user id, when reported.
    #[serde(default)]
    pub owner_id: Option<i64>,
    /// Total object count.
    #[serde(default)]
    pub objects_count: Option<i64>,
    /// Total stored bytes.
    #[serde(default)]
    pub objects_size: Option<i64>,
    /// Creation timestamp.
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Body for `POST …/region/{regionName}/storage` (`cloud.StorageContainerCreation`).
///
/// Only `name` is required. When `owner_id` is omitted OVH assigns ownership, and
/// any project user holding the `objectstore_operator` role can use the bucket
/// regardless of owner — so the bucket can be created before its S3 user exists.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageContainerCreate {
    /// Container (bucket) name.
    pub name: String,
    /// Optional owning project user id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<i64>,
}

impl OvhClient {
    /// `GET /cloud/project/{serviceName}/region/{regionName}/storage` — list S3
    /// containers in a region.
    pub async fn cloud_storage_containers(
        &self,
        service_name: &str,
        region_name: &str,
    ) -> Result<Vec<StorageContainer>> {
        let path = region_path(service_name, region_name, self, "/storage");
        self.get_v1(&path).await
    }

    /// `POST /cloud/project/{serviceName}/region/{regionName}/storage` — create an
    /// S3 container.
    pub async fn cloud_storage_container_create(
        &self,
        service_name: &str,
        region_name: &str,
        create: &StorageContainerCreate,
    ) -> Result<StorageContainer> {
        let path = region_path(service_name, region_name, self, "/storage");
        self.post_v1(&path, create).await
    }

    /// `GET /cloud/project/{serviceName}/region/{regionName}/storage/{name}` —
    /// container detail.
    pub async fn cloud_storage_container(
        &self,
        service_name: &str,
        region_name: &str,
        name: &str,
    ) -> Result<StorageContainer> {
        let path = format!(
            "{}/{}",
            region_path(service_name, region_name, self, "/storage"),
            self.encode_segment(name)
        );
        self.get_v1(&path).await
    }

    /// `DELETE /cloud/project/{serviceName}/region/{regionName}/storage/{name}`.
    pub async fn cloud_storage_container_delete(
        &self,
        service_name: &str,
        region_name: &str,
        name: &str,
    ) -> Result<()> {
        let path = format!(
            "{}/{}",
            region_path(service_name, region_name, self, "/storage"),
            self.encode_segment(name)
        );
        self.delete_v1(&path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_storage_container_list_entry() {
        let c: StorageContainer = serde_json::from_str(
            r#"{
                "name": "infrazeug-cnpg-backup",
                "region": "DE",
                "virtualHost": "infrazeug-cnpg-backup.s3.de.io.cloud.ovh.net",
                "ownerId": 1234,
                "objectsCount": 3,
                "objectsSize": 1024,
                "createdAt": "2024-01-01T00:00:00+00:00"
            }"#,
        )
        .unwrap();
        assert_eq!(c.name, "infrazeug-cnpg-backup");
        assert_eq!(c.region.as_deref(), Some("DE"));
    }

    #[test]
    fn deserializes_minimal_container() {
        // A container detail with only the name present must still decode.
        let c: StorageContainer = serde_json::from_str(r#"{"name": "backups"}"#).unwrap();
        assert_eq!(c.name, "backups");
        assert!(c.region.is_none());
    }

    #[test]
    fn serializes_create_without_owner() {
        let body = StorageContainerCreate {
            name: "backups".into(),
            owner_id: None,
        };
        let json = serde_json::to_string(&body).unwrap();
        assert_eq!(json, r#"{"name":"backups"}"#);
    }
}
