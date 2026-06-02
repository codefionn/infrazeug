//! Zone IP access rules (`/zones/{zone_id}/firewall/access_rules/rules`).

use crate::client::CloudflareClient;
use crate::error::Result;
use crate::types::ListQuery;
use serde::{Deserialize, Serialize};

/// Target selector for an IP access rule.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AccessRuleConfiguration {
    pub target: String,
    pub value: String,
}

/// A zone-level IP access rule.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AccessRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub mode: String,
    pub configuration: AccessRuleConfiguration,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_on: Option<String>,
}

/// Query parameters for listing access rules.
#[derive(Debug, Clone, Default)]
pub struct AccessRuleListQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub mode: Option<String>,
    pub configuration_target: Option<String>,
    pub configuration_value: Option<String>,
    pub notes: Option<String>,
}

impl AccessRuleListQuery {
    pub fn as_params(&self) -> Vec<(&str, String)> {
        let mut out = Vec::new();
        if let Some(page) = self.page {
            out.push(("page", page.to_string()));
        }
        if let Some(per_page) = self.per_page {
            out.push(("per_page", per_page.to_string()));
        }
        if let Some(mode) = &self.mode {
            out.push(("mode", mode.clone()));
        }
        if let Some(target) = &self.configuration_target {
            out.push(("configuration.target", target.clone()));
        }
        if let Some(value) = &self.configuration_value {
            out.push(("configuration.value", value.clone()));
        }
        if let Some(notes) = &self.notes {
            out.push(("notes", notes.clone()));
        }
        out
    }
}

impl CloudflareClient {
    /// `GET /zones/{zone_id}/firewall/access_rules/rules` — list access rules (all pages).
    pub async fn firewall_access_rules(
        &self,
        zone_id: &str,
        query: &AccessRuleListQuery,
    ) -> Result<Vec<AccessRule>> {
        let path = format!(
            "/zones/{}/firewall/access_rules/rules",
            self.encode_path(zone_id)
        );
        self.get_all_with_params(&path, query.as_params()).await
    }

    /// `GET /zones/{zone_id}/firewall/access_rules/rules/{id}` — fetch one rule.
    pub async fn firewall_access_rule(&self, zone_id: &str, rule_id: &str) -> Result<AccessRule> {
        let path = format!(
            "/zones/{}/firewall/access_rules/rules/{}",
            self.encode_path(zone_id),
            self.encode_path(rule_id)
        );
        let (rule, _) = self.get(&path, &ListQuery::default()).await?;
        Ok(rule)
    }

    /// `POST /zones/{zone_id}/firewall/access_rules/rules` — create an access rule.
    pub async fn create_firewall_access_rule(
        &self,
        zone_id: &str,
        body: &AccessRule,
    ) -> Result<AccessRule> {
        let path = format!(
            "/zones/{}/firewall/access_rules/rules",
            self.encode_path(zone_id)
        );
        self.post_json(&path, body).await
    }

    /// `PATCH /zones/{zone_id}/firewall/access_rules/rules/{id}` — update an access rule.
    pub async fn update_firewall_access_rule(
        &self,
        zone_id: &str,
        rule_id: &str,
        body: &AccessRule,
    ) -> Result<AccessRule> {
        let path = format!(
            "/zones/{}/firewall/access_rules/rules/{}",
            self.encode_path(zone_id),
            self.encode_path(rule_id)
        );
        self.patch_json(&path, body).await
    }

    /// `DELETE /zones/{zone_id}/firewall/access_rules/rules/{id}` — delete an access rule.
    pub async fn delete_firewall_access_rule(&self, zone_id: &str, rule_id: &str) -> Result<()> {
        let path = format!(
            "/zones/{}/firewall/access_rules/rules/{}",
            self.encode_path(zone_id),
            self.encode_path(rule_id)
        );
        self.delete(&path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::decode_body;
    use reqwest::StatusCode;

    #[test]
    fn decode_access_rule() {
        let body = r#"{
            "success": true,
            "errors": [],
            "messages": [],
            "result": {
                "id": "92f17202ed8bd63d69a66b86a49a8f6b",
                "mode": "block",
                "configuration": {
                    "target": "ip",
                    "value": "198.51.100.4"
                },
                "notes": "block scanner"
            }
        }"#;
        let (rule, _): (AccessRule, _) = decode_body(StatusCode::OK, body).unwrap();
        assert_eq!(rule.mode, "block");
        assert_eq!(rule.configuration.target, "ip");
        assert_eq!(rule.configuration.value, "198.51.100.4");
    }

    #[test]
    fn list_query_maps_configuration_filters() {
        let query = AccessRuleListQuery {
            mode: Some("block".into()),
            configuration_target: Some("ip".into()),
            configuration_value: Some("1.2.3.4".into()),
            ..Default::default()
        };
        let params = query.as_params();
        assert!(params.contains(&("mode", "block".into())));
        assert!(params.contains(&("configuration.target", "ip".into())));
        assert!(params.contains(&("configuration.value", "1.2.3.4".into())));
    }

    #[test]
    fn access_rule_matches_key() {
        let rule = AccessRule {
            mode: "whitelist".into(),
            configuration: AccessRuleConfiguration {
                target: "ip_range".into(),
                value: "192.0.2.0/24".into(),
            },
            ..Default::default()
        };
        assert_eq!(rule.mode, "whitelist");
        assert_eq!(rule.configuration.value, "192.0.2.0/24");
    }
}
