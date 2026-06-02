//! Image management (`/images`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};

/// Image resource returned by the Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Image {
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
    pub properties: Option<ImageProperties>,
}

/// Image properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImageProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licence_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_aliases: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_init: Option<String>,
}

/// Payload for updating an image.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImageUpdate {
    pub properties: ImageUpdateProperties,
}

/// Properties accepted when updating an image.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImageUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licence_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_init: Option<String>,
}

impl IonosClient {
    /// `GET /images` — list images in the current contract.
    pub async fn images(&self, query: &ListQuery) -> Result<Collection<Image>> {
        self.get("/images", query).await
    }

    /// `GET /images/{id}` — retrieve one image.
    pub async fn image(&self, image_id: &str, query: &ListQuery) -> Result<Image> {
        self.get(&format!("/images/{}", self.encode_path(image_id)), query)
            .await
    }

    /// `PUT /images/{id}` — update an image.
    pub async fn update_image(
        &self,
        image_id: &str,
        body: &ImageUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<Image> {
        self.put_json(
            &format!("/images/{}", self.encode_path(image_id)),
            body,
            query,
            etag,
        )
        .await
    }

    /// `DELETE /images/{id}` — delete an image.
    pub async fn delete_image(
        &self,
        image_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &format!("/images/{}", self.encode_path(image_id)),
            query,
            etag,
        )
        .await
    }
}
