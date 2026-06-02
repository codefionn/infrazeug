//! NIC management (`/datacenters/{dc}/servers/{srv}/nics`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// NIC resource returned by the Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Nic {
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
    pub properties: Option<NicProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// NIC properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NicProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ips: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6_ips: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6_cidr_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcpv6: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lan: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firewall_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firewall_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pci_slot: Option<u32>,
}

/// Payload for creating a NIC.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NicCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ElementMetadata>,
    pub properties: NicCreateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties for creating a NIC.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NicCreateProperties {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ips: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lan: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firewall_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firewall_type: Option<String>,
}

/// Payload for updating a NIC.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NicUpdate {
    pub properties: NicUpdateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties for updating a NIC.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NicUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ips: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lan: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firewall_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firewall_type: Option<String>,
}

fn nic_path(
    client: &IonosClient,
    datacenter_id: &str,
    server_id: &str,
    nic_id: Option<&str>,
) -> String {
    let mut path = format!(
        "/datacenters/{}/servers/{}/nics",
        client.encode_path(datacenter_id),
        client.encode_path(server_id)
    );
    if let Some(nic_id) = nic_id {
        path.push('/');
        path.push_str(&client.encode_path(nic_id));
    }
    path
}

impl IonosClient {
    /// `GET /datacenters/{dc}/servers/{srv}/nics` — list NICs on a server.
    pub async fn nics(
        &self,
        datacenter_id: &str,
        server_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<Nic>> {
        self.get(&nic_path(self, datacenter_id, server_id, None), query)
            .await
    }

    /// `GET /datacenters/{dc}/servers/{srv}/nics/{id}` — retrieve one NIC.
    pub async fn nic(
        &self,
        datacenter_id: &str,
        server_id: &str,
        nic_id: &str,
        query: &ListQuery,
    ) -> Result<Nic> {
        self.get(
            &nic_path(self, datacenter_id, server_id, Some(nic_id)),
            query,
        )
        .await
    }

    /// `POST /datacenters/{dc}/servers/{srv}/nics` — create a NIC.
    pub async fn create_nic(
        &self,
        datacenter_id: &str,
        server_id: &str,
        body: &NicCreate,
        query: &ListQuery,
    ) -> Result<Nic> {
        self.post_json(&nic_path(self, datacenter_id, server_id, None), body, query)
            .await
    }

    /// `PUT /datacenters/{dc}/servers/{srv}/nics/{id}` — update a NIC.
    pub async fn update_nic(
        &self,
        datacenter_id: &str,
        server_id: &str,
        nic_id: &str,
        body: &NicUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<Nic> {
        self.put_json(
            &nic_path(self, datacenter_id, server_id, Some(nic_id)),
            body,
            query,
            etag,
        )
        .await
    }

    /// `DELETE /datacenters/{dc}/servers/{srv}/nics/{id}` — delete a NIC.
    pub async fn delete_nic(
        &self,
        datacenter_id: &str,
        server_id: &str,
        nic_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &nic_path(self, datacenter_id, server_id, Some(nic_id)),
            query,
            etag,
        )
        .await
    }
}
