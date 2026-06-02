//! Rulesets API (`/zones/{zone_id}/rulesets`).

use crate::client::CloudflareClient;
use crate::error::Result;
use crate::types::ListQuery;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Phase names used by infrazeug helpers.
pub mod phase {
    pub const HTTP_REQUEST_FIREWALL_CUSTOM: &str = "http_request_firewall_custom";
    pub const HTTP_REQUEST_DYNAMIC_REDIRECT: &str = "http_request_dynamic_redirect";
}

/// A ruleset or phase entry point.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Ruleset {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<RulesetRule>>,
}

/// One rule inside a ruleset.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RulesetRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_parameters: Option<Value>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl RulesetRule {
    /// Stable reference tag when present (`ref` field).
    pub fn reference(&self) -> Option<&str> {
        self.ref_.as_deref()
    }
}

/// Body for creating a zone ruleset.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RulesetCreate {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub kind: String,
    pub phase: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<RulesetRule>>,
}

/// Body for replacing rules on an existing ruleset.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RulesetUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<RulesetRule>>,
}

impl CloudflareClient {
    /// `GET /zones/{zone_id}/rulesets` — list rulesets in a zone.
    pub async fn rulesets(&self, zone_id: &str) -> Result<Vec<Ruleset>> {
        let path = format!("/zones/{}/rulesets", self.encode_path(zone_id));
        self.get_all(&path, ListQuery::default()).await
    }

    /// `GET /zones/{zone_id}/rulesets/{ruleset_id}` — fetch one ruleset.
    pub async fn ruleset(&self, zone_id: &str, ruleset_id: &str) -> Result<Ruleset> {
        let path = format!(
            "/zones/{}/rulesets/{}",
            self.encode_path(zone_id),
            self.encode_path(ruleset_id)
        );
        let (ruleset, _) = self.get(&path, &ListQuery::default()).await?;
        Ok(ruleset)
    }

    /// `POST /zones/{zone_id}/rulesets` — create a ruleset.
    pub async fn create_ruleset(&self, zone_id: &str, body: &RulesetCreate) -> Result<Ruleset> {
        let path = format!("/zones/{}/rulesets", self.encode_path(zone_id));
        self.post_json(&path, body).await
    }

    /// `PUT /zones/{zone_id}/rulesets/{ruleset_id}` — replace a ruleset.
    pub async fn update_ruleset(
        &self,
        zone_id: &str,
        ruleset_id: &str,
        body: &RulesetUpdate,
    ) -> Result<Ruleset> {
        let path = format!(
            "/zones/{}/rulesets/{}",
            self.encode_path(zone_id),
            self.encode_path(ruleset_id)
        );
        self.put_json(&path, body).await
    }

    /// `POST /zones/{zone_id}/rulesets/{ruleset_id}/rules` — add a rule.
    pub async fn create_ruleset_rule(
        &self,
        zone_id: &str,
        ruleset_id: &str,
        body: &RulesetRule,
    ) -> Result<RulesetRule> {
        let path = format!(
            "/zones/{}/rulesets/{}/rules",
            self.encode_path(zone_id),
            self.encode_path(ruleset_id)
        );
        self.post_json(&path, body).await
    }

    /// `PATCH /zones/{zone_id}/rulesets/{ruleset_id}/rules/{rule_id}` — update a rule.
    pub async fn update_ruleset_rule(
        &self,
        zone_id: &str,
        ruleset_id: &str,
        rule_id: &str,
        body: &RulesetRule,
    ) -> Result<RulesetRule> {
        let path = format!(
            "/zones/{}/rulesets/{}/rules/{}",
            self.encode_path(zone_id),
            self.encode_path(ruleset_id),
            self.encode_path(rule_id)
        );
        self.patch_json(&path, body).await
    }

    /// `GET /zones/{zone_id}/rulesets/phases/{phase}/entrypoint` — fetch phase entry point.
    pub async fn phase_entrypoint_ruleset(&self, zone_id: &str, phase: &str) -> Result<Ruleset> {
        let path = format!(
            "/zones/{}/rulesets/phases/{}/entrypoint",
            self.encode_path(zone_id),
            self.encode_path(phase)
        );
        let (ruleset, _) = self.get(&path, &ListQuery::default()).await?;
        Ok(ruleset)
    }

    /// `PUT /zones/{zone_id}/rulesets/phases/{phase}/entrypoint` — replace phase entry point.
    pub async fn update_phase_entrypoint_ruleset(
        &self,
        zone_id: &str,
        phase: &str,
        body: &RulesetUpdate,
    ) -> Result<Ruleset> {
        let path = format!(
            "/zones/{}/rulesets/phases/{}/entrypoint",
            self.encode_path(zone_id),
            self.encode_path(phase)
        );
        self.put_json(&path, body).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::decode_body;
    use reqwest::StatusCode;

    #[test]
    fn decode_custom_waf_ruleset() {
        let body = r#"{
            "success": true,
            "errors": [],
            "messages": [],
            "result": {
                "id": "f82ccda3d21f4a02825d3fe45b5e1c10",
                "name": "infrazeug-managed-waf",
                "kind": "zone",
                "phase": "http_request_firewall_custom",
                "rules": [
                    {
                        "id": "abc",
                        "description": "block admin path",
                        "expression": "(http.request.uri.path eq \"/admin\")",
                        "action": "block",
                        "enabled": true
                    }
                ]
            }
        }"#;
        let (ruleset, _): (Ruleset, _) = decode_body(StatusCode::OK, body).unwrap();
        assert_eq!(
            ruleset.phase.as_deref(),
            Some("http_request_firewall_custom")
        );
        let rules = ruleset.rules.unwrap();
        assert_eq!(rules[0].description.as_deref(), Some("block admin path"));
        assert_eq!(rules[0].action.as_deref(), Some("block"));
    }

    #[test]
    fn decode_redirect_rule() {
        let body = r#"{
            "success": true,
            "errors": [],
            "messages": [],
            "result": {
                "action": "redirect",
                "description": "apex to www",
                "expression": "(http.host eq \"example.com\")",
                "action_parameters": {
                    "from_value": {
                        "status_code": 301,
                        "target_url": { "value": "https://www.example.com" },
                        "preserve_query_string": true
                    }
                }
            }
        }"#;
        let (rule, _): (RulesetRule, _) = decode_body(StatusCode::OK, body).unwrap();
        assert_eq!(rule.action.as_deref(), Some("redirect"));
        assert!(rule.action_parameters.is_some());
    }

    #[test]
    fn rule_reference_reads_ref_field() {
        let rule = RulesetRule {
            ref_: Some("my-ref".into()),
            ..Default::default()
        };
        assert_eq!(rule.reference(), Some("my-ref"));
    }
}
