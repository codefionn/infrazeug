//! Firewall groups — reusable address / port sets (`/rest/firewallgroup`).

use crate::client::{first_item, UnifiClient};
use crate::error::Result;
use serde::{Deserialize, Serialize};

const RESOURCE: &str = "firewallgroup";

/// A firewall group: a named set of addresses or ports referenced by rules.
///
/// Unmodelled fields round-trip through [`extra`](Self::extra).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FirewallGroup {
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    /// `address-group`, `port-group`, or `ipv6-address-group`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_type: Option<String>,
    /// Members — IPs/CIDRs for address groups, port numbers for port groups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_members: Option<Vec<String>>,
    /// Fields this client does not model, preserved verbatim across updates.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl UnifiClient {
    /// `GET /rest/firewallgroup` — list firewall groups on the site.
    pub async fn firewall_groups(&self) -> Result<Vec<FirewallGroup>> {
        self.rest_list(RESOURCE).await
    }

    /// `POST /rest/firewallgroup` — create a firewall group.
    pub async fn create_firewall_group(&self, body: &FirewallGroup) -> Result<FirewallGroup> {
        first_item(self.rest_create(RESOURCE, body).await?, "firewall group")
    }

    /// `PUT /rest/firewallgroup/{id}` — replace a firewall group.
    pub async fn update_firewall_group(
        &self,
        id: &str,
        body: &FirewallGroup,
    ) -> Result<FirewallGroup> {
        first_item(
            self.rest_update(RESOURCE, id, body).await?,
            "firewall group",
        )
    }

    /// `DELETE /rest/firewallgroup/{id}` — delete a firewall group.
    pub async fn delete_firewall_group(&self, id: &str) -> Result<()> {
        self.rest_delete(RESOURCE, id).await
    }
}
