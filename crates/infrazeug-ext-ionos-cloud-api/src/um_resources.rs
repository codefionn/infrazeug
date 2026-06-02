//! UM resource inventory (`/um/resources`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ListQuery};

pub use crate::um_types::{UmResource, UmResourceType};

impl IonosClient {
    /// `GET /um/resources` — list all available resources.
    pub async fn um_resources(&self, query: &ListQuery) -> Result<Collection<UmResource>> {
        self.get("/um/resources", query).await
    }

    /// `GET /um/resources/{type}` — list resources of a given type.
    pub async fn um_resources_by_type(
        &self,
        resource_type: UmResourceType,
        query: &ListQuery,
    ) -> Result<Collection<UmResource>> {
        let segment = serde_json::to_value(resource_type)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "datacenter".into());
        self.get(
            &format!("/um/resources/{}", self.encode_path(&segment)),
            query,
        )
        .await
    }

    /// `GET /um/resources/{type}/{id}` — retrieve one resource by type and ID.
    pub async fn um_resource(
        &self,
        resource_type: UmResourceType,
        resource_id: &str,
        query: &ListQuery,
    ) -> Result<UmResource> {
        let segment = serde_json::to_value(resource_type)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "datacenter".into());
        self.get(
            &format!(
                "/um/resources/{}/{}",
                self.encode_path(&segment),
                self.encode_path(resource_id)
            ),
            query,
        )
        .await
    }
}
