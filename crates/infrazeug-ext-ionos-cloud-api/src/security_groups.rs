//! Security group management.

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Security group resource.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroup {
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
    pub properties: Option<SecurityGroupProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Security group properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Payload for creating a security group.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupCreate {
    pub properties: SecurityGroupCreateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties for creating a security group.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupCreateProperties {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Payload for updating a security group.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupUpdate {
    pub properties: SecurityGroupUpdateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties for updating a security group.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Security group firewall rule (reuses firewall rule shape).
pub use crate::firewall_rules::{FirewallRule, FirewallRuleCreate, FirewallRuleUpdate};

fn security_group_path(
    client: &IonosClient,
    datacenter_id: &str,
    group_id: Option<&str>,
) -> String {
    let mut path = format!(
        "/datacenters/{}/securitygroups",
        client.encode_path(datacenter_id)
    );
    if let Some(group_id) = group_id {
        path.push('/');
        path.push_str(&client.encode_path(group_id));
    }
    path
}

fn security_group_rule_path(
    client: &IonosClient,
    datacenter_id: &str,
    group_id: &str,
    rule_id: Option<&str>,
) -> String {
    let mut path = format!(
        "{}/rules",
        security_group_path(client, datacenter_id, Some(group_id))
    );
    if let Some(rule_id) = rule_id {
        path.push('/');
        path.push_str(&client.encode_path(rule_id));
    }
    path
}

impl IonosClient {
    /// `GET /datacenters/{dc}/securitygroups` — list security groups.
    pub async fn security_groups(
        &self,
        datacenter_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<SecurityGroup>> {
        self.get(&security_group_path(self, datacenter_id, None), query)
            .await
    }

    /// Retrieve one security group.
    pub async fn security_group(
        &self,
        datacenter_id: &str,
        group_id: &str,
        query: &ListQuery,
    ) -> Result<SecurityGroup> {
        self.get(
            &security_group_path(self, datacenter_id, Some(group_id)),
            query,
        )
        .await
    }

    /// Create a security group.
    pub async fn create_security_group(
        &self,
        datacenter_id: &str,
        body: &SecurityGroupCreate,
        query: &ListQuery,
    ) -> Result<SecurityGroup> {
        self.post_json(&security_group_path(self, datacenter_id, None), body, query)
            .await
    }

    /// Update a security group.
    pub async fn update_security_group(
        &self,
        datacenter_id: &str,
        group_id: &str,
        body: &SecurityGroupUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<SecurityGroup> {
        self.put_json(
            &security_group_path(self, datacenter_id, Some(group_id)),
            body,
            query,
            etag,
        )
        .await
    }

    /// Delete a security group.
    pub async fn delete_security_group(
        &self,
        datacenter_id: &str,
        group_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &security_group_path(self, datacenter_id, Some(group_id)),
            query,
            etag,
        )
        .await
    }

    /// List rules in a security group.
    pub async fn security_group_rules(
        &self,
        datacenter_id: &str,
        group_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<FirewallRule>> {
        self.get(
            &security_group_rule_path(self, datacenter_id, group_id, None),
            query,
        )
        .await
    }

    /// Create a rule in a security group.
    pub async fn create_security_group_rule(
        &self,
        datacenter_id: &str,
        group_id: &str,
        body: &FirewallRuleCreate,
        query: &ListQuery,
    ) -> Result<FirewallRule> {
        self.post_json(
            &security_group_rule_path(self, datacenter_id, group_id, None),
            body,
            query,
        )
        .await
    }

    /// Delete a rule from a security group.
    pub async fn delete_security_group_rule(
        &self,
        datacenter_id: &str,
        group_id: &str,
        rule_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &security_group_rule_path(self, datacenter_id, group_id, Some(rule_id)),
            query,
            etag,
        )
        .await
    }

    /// Attach security groups to a server.
    pub async fn attach_server_security_groups(
        &self,
        datacenter_id: &str,
        server_id: &str,
        group_ids: &[String],
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        let body = serde_json::json!({
            "id": serde_json::Value::Null,
            "type": "security-groups",
            "properties": {},
            "entities": {
                "securitygroups": {
                    "items": group_ids.iter().map(|id| serde_json::json!({
                        "id": id,
                        "type": "security-group"
                    })).collect::<Vec<_>>()
                }
            }
        });
        self.put(
            &format!(
                "/datacenters/{}/servers/{}/securitygroups",
                self.encode_path(datacenter_id),
                self.encode_path(server_id)
            ),
            &body,
            query,
            etag,
        )
        .await
    }
}
