//! Ensure a firewall rule exists and matches a small set of managed fields.

use crate::client::UnifiClientSource;
use async_trait::async_trait;
use infrazeug_ext_unifi_api::firewall_rule::FirewallRule;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_FIREWALL_RULE: &str = "unifi.ensure_firewall_rule";

/// Tier-1 method: ensure a firewall rule.
pub type EnsureFirewallRule = EnsureResource<FirewallRuleResource>;

/// Construct the registrable [`EnsureFirewallRule`] method for a client source.
pub fn ensure_firewall_rule(source: UnifiClientSource) -> EnsureFirewallRule {
    EnsureResource::new(FirewallRuleResource::new(source))
}

fn default_action() -> String {
    "drop".into()
}
fn default_protocol() -> String {
    "all".into()
}

/// Desired firewall rule. The name is the natural key.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureFirewallRuleInput {
    pub name: String,
    /// Ruleset, e.g. `WAN_IN`, `LAN_IN`, `LAN_LOCAL`, `GUEST_IN`.
    pub ruleset: String,
    /// Ordering index within the ruleset (e.g. `2000`).
    pub rule_index: u32,
    /// `accept`, `drop` (default), or `reject`.
    #[serde(default = "default_action")]
    pub action: String,
    /// `all` (default), `tcp`, `udp`, `tcp_udp`, `icmp`, …
    #[serde(default = "default_protocol")]
    pub protocol: String,
    /// Defaults to `true` on create when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<bool>,
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
}

/// Observed firewall rule — managed fields plus the controller id.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureFirewallRuleOutput {
    pub rule_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruleset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// A firewall rule as an acquirable resource.
#[derive(Clone)]
pub struct FirewallRuleResource {
    source: UnifiClientSource,
}

impl FirewallRuleResource {
    pub fn new(source: UnifiClientSource) -> Self {
        Self { source }
    }

    async fn find(&self, ctx: &ResourceCtx, name: &str) -> ResourceResult<Option<FirewallRule>> {
        let client = self.source.client(ctx).await?;
        let rules = client
            .firewall_rules()
            .await
            .map_err(ResourceError::provider)?;
        Ok(rules.into_iter().find(|r| r.name == name))
    }
}

fn to_output(rule: FirewallRule) -> Option<EnsureFirewallRuleOutput> {
    let id = rule.id?;
    Some(EnsureFirewallRuleOutput {
        rule_id: id,
        name: rule.name,
        ruleset: rule.ruleset,
        rule_index: rule.rule_index,
        action: rule.action,
        protocol: rule.protocol,
        enabled: rule.enabled,
    })
}

fn overlay(rule: &mut FirewallRule, spec: &EnsureFirewallRuleInput) {
    rule.name = spec.name.clone();
    rule.ruleset = Some(spec.ruleset.clone());
    rule.rule_index = Some(spec.rule_index);
    rule.action = Some(spec.action.clone());
    rule.protocol = Some(spec.protocol.clone());
    rule.enabled = Some(spec.enabled.unwrap_or(true));
    if spec.logging.is_some() {
        rule.logging = spec.logging;
    }
    if spec.src_firewallgroup_ids.is_some() {
        rule.src_firewallgroup_ids = spec.src_firewallgroup_ids.clone();
    }
    if spec.dst_firewallgroup_ids.is_some() {
        rule.dst_firewallgroup_ids = spec.dst_firewallgroup_ids.clone();
    }
    if spec.src_address.is_some() {
        rule.src_address = spec.src_address.clone();
    }
    if spec.dst_address.is_some() {
        rule.dst_address = spec.dst_address.clone();
    }
    if spec.src_port.is_some() {
        rule.src_port = spec.src_port.clone();
    }
    if spec.dst_port.is_some() {
        rule.dst_port = spec.dst_port.clone();
    }
}

#[async_trait]
impl Resource for FirewallRuleResource {
    type Spec = EnsureFirewallRuleInput;
    type State = EnsureFirewallRuleOutput;

    fn kind(&self) -> &'static str {
        ENSURE_FIREWALL_RULE
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        Ok(self.find(ctx, &spec.name).await?.and_then(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let mut rule = FirewallRule::default();
        overlay(&mut rule, spec);
        let created = client
            .create_firewall_rule(&rule)
            .await
            .map_err(ResourceError::provider)?;
        to_output(created).ok_or_else(|| ResourceError::provider("created firewall rule has no id"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if current.ruleset.as_deref() != Some(spec.ruleset.as_str()) {
            diffs.push(format!(
                "ruleset {:?} → {:?}",
                current.ruleset, spec.ruleset
            ));
        }
        if current.rule_index != Some(spec.rule_index) {
            diffs.push(format!(
                "rule_index {:?} → {}",
                current.rule_index, spec.rule_index
            ));
        }
        if current.action.as_deref() != Some(spec.action.as_str()) {
            diffs.push(format!("action {:?} → {:?}", current.action, spec.action));
        }
        if current.protocol.as_deref() != Some(spec.protocol.as_str()) {
            diffs.push(format!(
                "protocol {:?} → {:?}",
                current.protocol, spec.protocol
            ));
        }
        if let Some(enabled) = spec.enabled {
            if current.enabled != Some(enabled) {
                diffs.push(format!("enabled {:?} → {}", current.enabled, enabled));
            }
        }
        if diffs.is_empty() {
            Drift::InSync
        } else {
            Drift::Drifted(diffs.join(", "))
        }
    }

    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let mut rule = self
            .find(ctx, &spec.name)
            .await?
            .ok_or_else(|| ResourceError::provider("firewall rule disappeared before reconcile"))?;
        overlay(&mut rule, spec);
        let updated = client
            .update_firewall_rule(&current.rule_id, &rule)
            .await
            .map_err(ResourceError::provider)?;
        to_output(updated).ok_or_else(|| ResourceError::provider("updated firewall rule has no id"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> FirewallRuleResource {
        FirewallRuleResource::new(UnifiClientSource::vault(
            "https://unifi.local",
            "unifi.vault",
        ))
    }

    fn spec() -> EnsureFirewallRuleInput {
        EnsureFirewallRuleInput {
            name: "block-iot-wan".into(),
            ruleset: "LAN_IN".into(),
            rule_index: 2000,
            action: "drop".into(),
            protocol: "all".into(),
            enabled: Some(true),
            ..Default::default()
        }
    }

    fn current() -> EnsureFirewallRuleOutput {
        EnsureFirewallRuleOutput {
            rule_id: "fr-1".into(),
            name: "block-iot-wan".into(),
            ruleset: Some("LAN_IN".into()),
            rule_index: Some(2000),
            action: Some("drop".into()),
            protocol: Some("all".into()),
            enabled: Some(true),
        }
    }

    #[test]
    fn matching_spec_is_in_sync() {
        assert_eq!(resource().diff(&spec(), &current()), Drift::InSync);
    }

    #[test]
    fn changed_action_drifts() {
        let mut s = spec();
        s.action = "accept".into();
        assert!(matches!(resource().diff(&s, &current()), Drift::Drifted(_)));
    }
}
