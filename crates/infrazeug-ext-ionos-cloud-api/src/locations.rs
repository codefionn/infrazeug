//! Location discovery (`/locations`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};

/// Location resource returned by the Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ElementMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<LocationProperties>,
}

/// Location properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LocationProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_aliases: Option<Vec<String>>,
}

impl IonosClient {
    /// `GET /locations` — list all locations.
    pub async fn locations(&self, query: &ListQuery) -> Result<Collection<Location>> {
        self.get("/locations", query).await
    }

    /// `GET /locations/{regionId}` — list locations within a region.
    pub async fn locations_in_region(
        &self,
        region_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<Location>> {
        self.get(
            &format!("/locations/{}", self.encode_path(region_id)),
            query,
        )
        .await
    }

    /// `GET /locations/{regionId}/{locationId}` — retrieve one location.
    pub async fn location(
        &self,
        region_id: &str,
        location_id: &str,
        query: &ListQuery,
    ) -> Result<Location> {
        self.get(
            &format!(
                "/locations/{}/{}",
                self.encode_path(region_id),
                self.encode_path(location_id)
            ),
            query,
        )
        .await
    }
}
