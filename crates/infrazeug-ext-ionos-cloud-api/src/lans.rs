//! LAN management (`/datacenters/{id}/lans`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// LAN resource returned by the Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Lan {
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
    pub properties: Option<LanProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// LAN properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LanProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4_cidr_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6_cidr_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pcc: Option<String>,
}

/// Payload for creating a LAN.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LanCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    pub properties: LanCreateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties for creating a LAN.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LanCreateProperties {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4_cidr_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6_cidr_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
}

/// Payload for updating a LAN.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LanUpdate {
    pub properties: LanUpdateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties for updating a LAN.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LanUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4_cidr_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6_cidr_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
}

fn lan_path(client: &IonosClient, datacenter_id: &str, lan_id: Option<&str>) -> String {
    let mut path = format!("/datacenters/{}/lans", client.encode_path(datacenter_id));
    if let Some(lan_id) = lan_id {
        path.push('/');
        path.push_str(&client.encode_path(lan_id));
    }
    path
}

impl IonosClient {
    /// `GET /datacenters/{dc}/lans` — list LANs.
    pub async fn lans(&self, datacenter_id: &str, query: &ListQuery) -> Result<Collection<Lan>> {
        self.get(&lan_path(self, datacenter_id, None), query).await
    }

    /// `GET /datacenters/{dc}/lans/{id}` — retrieve one LAN.
    pub async fn lan(&self, datacenter_id: &str, lan_id: &str, query: &ListQuery) -> Result<Lan> {
        self.get(&lan_path(self, datacenter_id, Some(lan_id)), query)
            .await
    }

    /// `POST /datacenters/{dc}/lans` — create a LAN.
    pub async fn create_lan(
        &self,
        datacenter_id: &str,
        body: &LanCreate,
        query: &ListQuery,
    ) -> Result<Lan> {
        self.post_json(&lan_path(self, datacenter_id, None), body, query)
            .await
    }

    /// `PUT /datacenters/{dc}/lans/{id}` — update a LAN.
    pub async fn update_lan(
        &self,
        datacenter_id: &str,
        lan_id: &str,
        body: &LanUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<Lan> {
        self.put_json(
            &lan_path(self, datacenter_id, Some(lan_id)),
            body,
            query,
            etag,
        )
        .await
    }

    /// `DELETE /datacenters/{dc}/lans/{id}` — delete a LAN.
    pub async fn delete_lan(
        &self,
        datacenter_id: &str,
        lan_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(&lan_path(self, datacenter_id, Some(lan_id)), query, etag)
            .await
    }
}
