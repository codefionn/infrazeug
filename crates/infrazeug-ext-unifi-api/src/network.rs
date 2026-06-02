//! LAN / VLAN configuration (`/rest/networkconf`).

use crate::client::{first_item, UnifiClient};
use crate::error::Result;
use serde::{Deserialize, Serialize};

const RESOURCE: &str = "networkconf";

/// A network (LAN, VLAN-only network, guest network, …) as stored by the controller.
///
/// Unmodelled fields round-trip through [`extra`](Self::extra).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct NetworkConf {
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    /// `corporate`, `vlan-only`, `guest`, `wan`, …
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlan_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlan: Option<u16>,
    /// CIDR for the gateway interface (e.g. `10.0.5.1/24`) on routed networks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_subnet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dhcpd_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dhcpd_start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dhcpd_stop: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub networkgroup: Option<String>,
    /// Fields this client does not model, preserved verbatim across updates.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl UnifiClient {
    /// `GET /rest/networkconf` — list networks on the site.
    pub async fn networks(&self) -> Result<Vec<NetworkConf>> {
        self.rest_list(RESOURCE).await
    }

    /// `POST /rest/networkconf` — create a network.
    pub async fn create_network(&self, body: &NetworkConf) -> Result<NetworkConf> {
        first_item(self.rest_create(RESOURCE, body).await?, "network")
    }

    /// `PUT /rest/networkconf/{id}` — replace a network.
    pub async fn update_network(&self, id: &str, body: &NetworkConf) -> Result<NetworkConf> {
        first_item(self.rest_update(RESOURCE, id, body).await?, "network")
    }

    /// `DELETE /rest/networkconf/{id}` — delete a network.
    pub async fn delete_network(&self, id: &str) -> Result<()> {
        self.rest_delete(RESOURCE, id).await
    }
}
