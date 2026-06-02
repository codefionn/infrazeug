//! IP block management (`/ipblocks`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};

/// IP block resource returned by the Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IpBlock {
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
    pub properties: Option<IpBlockProperties>,
}

/// IP block properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IpBlockProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ips: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Payload for reserving an IP block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpBlockCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    pub properties: IpBlockCreateProperties,
}

/// Properties for reserving an IP block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpBlockCreateProperties {
    pub location: String,
    pub size: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Payload for updating an IP block.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IpBlockUpdate {
    pub properties: IpBlockUpdateProperties,
}

/// Properties accepted when updating an IP block.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IpBlockUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
}

impl IonosClient {
    /// `GET /ipblocks` — list reserved IP blocks.
    pub async fn ipblocks(&self, query: &ListQuery) -> Result<Collection<IpBlock>> {
        self.get("/ipblocks", query).await
    }

    /// `GET /ipblocks/{id}` — retrieve one IP block.
    pub async fn ipblock(&self, ipblock_id: &str, query: &ListQuery) -> Result<IpBlock> {
        self.get(
            &format!("/ipblocks/{}", self.encode_path(ipblock_id)),
            query,
        )
        .await
    }

    /// `POST /ipblocks` — reserve a new IP block.
    pub async fn create_ipblock(&self, body: &IpBlockCreate, query: &ListQuery) -> Result<IpBlock> {
        self.post_json("/ipblocks", body, query).await
    }

    /// `PUT /ipblocks/{id}` — update an IP block.
    pub async fn update_ipblock(
        &self,
        ipblock_id: &str,
        body: &IpBlockUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<IpBlock> {
        self.put_json(
            &format!("/ipblocks/{}", self.encode_path(ipblock_id)),
            body,
            query,
            etag,
        )
        .await
    }

    /// `DELETE /ipblocks/{id}` — delete an IP block.
    pub async fn delete_ipblock(
        &self,
        ipblock_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &format!("/ipblocks/{}", self.encode_path(ipblock_id)),
            query,
            etag,
        )
        .await
    }
}
