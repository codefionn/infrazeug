//! Ensure a RouterOS firewall filter rule exists and matches managed fields.

use crate::client::MikrotikClientSource;
use async_trait::async_trait;
use infrazeug_ext_mikrotik_api::firewall_filter::FirewallFilter;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_FIREWALL_RULE: &str = "mikrotik.ensure_firewall_rule";

/// Tier-1 method: ensure a firewall filter rule.
pub type EnsureFirewallRule = EnsureResource<FirewallRuleResource>;

/// Construct the registrable [`EnsureFirewallRule`] method for a client source.
pub fn ensure_firewall_rule(source: MikrotikClientSource) -> EnsureFirewallRule {
    EnsureResource::new(FirewallRuleResource::new(source))
}

fn default_action() -> String {
    "accept".into()
}
fn default_protocol() -> String {
    "tcp".into()
}

/// Desired firewall rule. Natural key: `comment` (RouterOS rule label).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureFirewallRuleInput {
    /// Stored as RouterOS `comment`.
    pub comment: String,
    /// e.g. `input`, `forward`, `output`.
    pub chain: String,
    /// `accept`, `drop`, `reject`, …
    #[serde(default = "default_action")]
    pub action: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_port: Option<String>,
    /// Defaults to `false` (enabled) on create when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

/// Observed firewall rule — managed fields plus the RouterOS `.id`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureFirewallRuleOutput {
    pub rule_id: String,
    pub comment: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

#[derive(Clone)]
pub struct FirewallRuleResource {
    source: MikrotikClientSource,
}

impl FirewallRuleResource {
    pub fn new(source: MikrotikClientSource) -> Self {
        Self { source }
    }

    async fn find(
        &self,
        ctx: &ResourceCtx,
        comment: &str,
    ) -> ResourceResult<Option<FirewallFilter>> {
        let params = self.source.params(ctx).await?;
        let mut client = params.connect().await.map_err(ResourceError::provider)?;
        let rules = client
            .firewall_filters()
            .await
            .map_err(ResourceError::provider)?;
        Ok(rules.into_iter().find(|r| r.comment == comment))
    }
}

fn to_output(rule: FirewallFilter) -> Option<EnsureFirewallRuleOutput> {
    let id = rule.id?;
    Some(EnsureFirewallRuleOutput {
        rule_id: id,
        comment: rule.comment,
        chain: rule.chain,
        action: rule.action,
        protocol: rule.protocol,
        disabled: rule.disabled,
    })
}

fn overlay(rule: &mut FirewallFilter, spec: &EnsureFirewallRuleInput) {
    rule.comment = spec.comment.clone();
    rule.chain = Some(spec.chain.clone());
    rule.action = Some(spec.action.clone());
    rule.protocol = Some(spec.protocol.clone());
    rule.src_address = spec.src_address.clone();
    rule.dst_address = spec.dst_address.clone();
    rule.src_port = spec.src_port.clone();
    rule.dst_port = spec.dst_port.clone();
    rule.disabled = Some(spec.disabled.unwrap_or(false));
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
        Ok(self.find(ctx, &spec.comment).await?.and_then(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let params = self.source.params(ctx).await?;
        let mut client = params.connect().await.map_err(ResourceError::provider)?;
        let mut rule = FirewallFilter::default();
        overlay(&mut rule, spec);
        let created = client
            .add_firewall_filter(&rule)
            .await
            .map_err(ResourceError::provider)?;
        to_output(created).ok_or_else(|| ResourceError::provider("created firewall rule has no id"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if current.chain.as_deref() != Some(spec.chain.as_str()) {
            diffs.push(format!("chain {:?} → {:?}", current.chain, spec.chain));
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
        if let Some(disabled) = spec.disabled {
            if current.disabled != Some(disabled) {
                diffs.push(format!("disabled {:?} → {}", current.disabled, disabled));
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
        let params = self.source.params(ctx).await?;
        let mut client = params.connect().await.map_err(ResourceError::provider)?;
        let mut rule = self
            .find(ctx, &spec.comment)
            .await?
            .ok_or_else(|| ResourceError::provider("firewall rule disappeared before reconcile"))?;
        overlay(&mut rule, spec);
        client
            .set_firewall_filter(&current.rule_id, &rule)
            .await
            .map_err(ResourceError::provider)?;
        to_output(rule).ok_or_else(|| ResourceError::provider("updated firewall rule has no id"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> FirewallRuleResource {
        FirewallRuleResource::new(MikrotikClientSource::vault(
            "192.168.88.1",
            "mikrotik.vault",
        ))
    }

    fn spec() -> EnsureFirewallRuleInput {
        EnsureFirewallRuleInput {
            comment: "allow-ssh".into(),
            chain: "input".into(),
            action: "accept".into(),
            protocol: "tcp".into(),
            dst_port: Some("22".into()),
            disabled: Some(false),
            ..Default::default()
        }
    }

    fn current() -> EnsureFirewallRuleOutput {
        EnsureFirewallRuleOutput {
            rule_id: "*5".into(),
            comment: "allow-ssh".into(),
            chain: Some("input".into()),
            action: Some("accept".into()),
            protocol: Some("tcp".into()),
            disabled: Some(false),
        }
    }

    #[test]
    fn matching_spec_is_in_sync() {
        assert_eq!(resource().diff(&spec(), &current()), Drift::InSync);
    }

    #[test]
    fn changed_action_drifts() {
        let mut s = spec();
        s.action = "drop".into();
        assert!(matches!(resource().diff(&s, &current()), Drift::Drifted(_)));
    }
}
