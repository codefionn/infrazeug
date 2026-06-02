//! NIC firewall rule management.

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};

/// Firewall rule resource.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FirewallRule {
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
    pub properties: Option<FirewallRuleProperties>,
}

/// Firewall rule properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FirewallRuleProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_mac: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icmp_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icmp_type: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_range_start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_range_end: Option<u32>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// Payload for creating a firewall rule.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FirewallRuleCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    pub properties: FirewallRuleCreateProperties,
}

/// Properties for creating a firewall rule.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FirewallRuleCreateProperties {
    pub name: String,
    pub protocol: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_range_start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_range_end: Option<u32>,
}

/// Payload for updating a firewall rule.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FirewallRuleUpdate {
    pub properties: FirewallRuleUpdateProperties,
}

/// Properties for updating a firewall rule.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FirewallRuleUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_range_start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_range_end: Option<u32>,
}

fn firewall_rule_path(
    client: &IonosClient,
    datacenter_id: &str,
    server_id: &str,
    nic_id: &str,
    rule_id: Option<&str>,
) -> String {
    let mut path = format!(
        "/datacenters/{}/servers/{}/nics/{}/firewallrules",
        client.encode_path(datacenter_id),
        client.encode_path(server_id),
        client.encode_path(nic_id)
    );
    if let Some(rule_id) = rule_id {
        path.push('/');
        path.push_str(&client.encode_path(rule_id));
    }
    path
}

impl IonosClient {
    /// List firewall rules on a NIC.
    pub async fn firewall_rules(
        &self,
        datacenter_id: &str,
        server_id: &str,
        nic_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<FirewallRule>> {
        self.get(
            &firewall_rule_path(self, datacenter_id, server_id, nic_id, None),
            query,
        )
        .await
    }

    /// Retrieve one firewall rule.
    pub async fn firewall_rule(
        &self,
        datacenter_id: &str,
        server_id: &str,
        nic_id: &str,
        rule_id: &str,
        query: &ListQuery,
    ) -> Result<FirewallRule> {
        self.get(
            &firewall_rule_path(self, datacenter_id, server_id, nic_id, Some(rule_id)),
            query,
        )
        .await
    }

    /// Create a firewall rule on a NIC.
    pub async fn create_firewall_rule(
        &self,
        datacenter_id: &str,
        server_id: &str,
        nic_id: &str,
        body: &FirewallRuleCreate,
        query: &ListQuery,
    ) -> Result<FirewallRule> {
        self.post_json(
            &firewall_rule_path(self, datacenter_id, server_id, nic_id, None),
            body,
            query,
        )
        .await
    }

    /// Update a firewall rule.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_firewall_rule(
        &self,
        datacenter_id: &str,
        server_id: &str,
        nic_id: &str,
        rule_id: &str,
        body: &FirewallRuleUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<FirewallRule> {
        self.put_json(
            &firewall_rule_path(self, datacenter_id, server_id, nic_id, Some(rule_id)),
            body,
            query,
            etag,
        )
        .await
    }

    /// Delete a firewall rule.
    pub async fn delete_firewall_rule(
        &self,
        datacenter_id: &str,
        server_id: &str,
        nic_id: &str,
        rule_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &firewall_rule_path(self, datacenter_id, server_id, nic_id, Some(rule_id)),
            query,
            etag,
        )
        .await
    }
}
