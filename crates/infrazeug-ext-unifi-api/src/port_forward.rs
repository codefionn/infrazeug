//! Port-forwarding rules (`/rest/portforward`).

use crate::client::{first_item, UnifiClient};
use crate::error::Result;
use serde::{Deserialize, Serialize};

const RESOURCE: &str = "portforward";

/// A port-forward (destination-NAT) rule as stored by the controller.
///
/// Ports are strings in the UniFi API (they may be ranges like `"8000-8010"`).
/// Unmodelled fields round-trip through [`extra`](Self::extra).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PortForward {
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// WAN interface the rule applies to (e.g. `wan`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pfwd_interface: Option<String>,
    /// Internal destination IP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fwd: Option<String>,
    /// Internal destination port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fwd_port: Option<String>,
    /// External (WAN) port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_port: Option<String>,
    /// `tcp`, `udp`, or `tcp_udp`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proto: Option<String>,
    /// Source restriction (`any` or a CIDR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<bool>,
    /// Fields this client does not model, preserved verbatim across updates.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl UnifiClient {
    /// `GET /rest/portforward` — list port-forward rules on the site.
    pub async fn port_forwards(&self) -> Result<Vec<PortForward>> {
        self.rest_list(RESOURCE).await
    }

    /// `POST /rest/portforward` — create a port-forward rule.
    pub async fn create_port_forward(&self, body: &PortForward) -> Result<PortForward> {
        first_item(self.rest_create(RESOURCE, body).await?, "port forward")
    }

    /// `PUT /rest/portforward/{id}` — replace a port-forward rule.
    pub async fn update_port_forward(&self, id: &str, body: &PortForward) -> Result<PortForward> {
        first_item(self.rest_update(RESOURCE, id, body).await?, "port forward")
    }

    /// `DELETE /rest/portforward/{id}` — delete a port-forward rule.
    pub async fn delete_port_forward(&self, id: &str) -> Result<()> {
        self.rest_delete(RESOURCE, id).await
    }
}
