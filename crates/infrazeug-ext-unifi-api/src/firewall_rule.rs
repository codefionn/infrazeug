//! Firewall rules (`/rest/firewallrule`).
//!
//! These are the classic per-ruleset firewall rules. Modern UniFi builds also offer
//! zone-based firewall policies; this binding targets the classic `firewallrule`
//! surface, which remains available on most controllers.

use crate::client::{first_item, UnifiClient};
use crate::error::Result;
use serde::{Deserialize, Serialize};

const RESOURCE: &str = "firewallrule";

/// A firewall rule within a ruleset.
///
/// Only commonly managed fields are typed; everything else round-trips through
/// [`extra`](Self::extra) so a read-modify-write update is non-destructive.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FirewallRule {
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// `accept`, `drop`, or `reject`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// Ruleset, e.g. `WAN_IN`, `LAN_IN`, `LAN_LOCAL`, `GUEST_IN`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruleset: Option<String>,
    /// Ordering index within the ruleset (e.g. `2000`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_index: Option<u32>,
    /// `all`, `tcp`, `udp`, `tcp_udp`, `icmp`, …
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_firewallgroup_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_firewallgroup_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<bool>,
    /// Fields this client does not model, preserved verbatim across updates.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl UnifiClient {
    /// `GET /rest/firewallrule` — list firewall rules on the site.
    pub async fn firewall_rules(&self) -> Result<Vec<FirewallRule>> {
        self.rest_list(RESOURCE).await
    }

    /// `POST /rest/firewallrule` — create a firewall rule.
    pub async fn create_firewall_rule(&self, body: &FirewallRule) -> Result<FirewallRule> {
        first_item(self.rest_create(RESOURCE, body).await?, "firewall rule")
    }

    /// `PUT /rest/firewallrule/{id}` — replace a firewall rule.
    pub async fn update_firewall_rule(
        &self,
        id: &str,
        body: &FirewallRule,
    ) -> Result<FirewallRule> {
        first_item(self.rest_update(RESOURCE, id, body).await?, "firewall rule")
    }

    /// `DELETE /rest/firewallrule/{id}` — delete a firewall rule.
    pub async fn delete_firewall_rule(&self, id: &str) -> Result<()> {
        self.rest_delete(RESOURCE, id).await
    }
}
