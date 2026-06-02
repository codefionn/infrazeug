//! Ensure a Cloudflare zone IP access rule exists and matches managed fields.

use crate::client::CloudflareClientSource;
use crate::methods::zone::resolve_zone_id;
use async_trait::async_trait;
use infrazeug_ext_cloudflare_api::firewall_access_rule::{
    AccessRule, AccessRuleConfiguration, AccessRuleListQuery,
};
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_FIREWALL_ACCESS_RULE: &str = "cloudflare.ensure_firewall_access_rule";

/// Tier-1 method: ensure a zone IP access rule.
pub type EnsureFirewallAccessRule = EnsureResource<FirewallAccessRuleResource>;

/// Construct the registrable [`EnsureFirewallAccessRule`] method for a client source.
pub fn ensure_firewall_access_rule(source: CloudflareClientSource) -> EnsureFirewallAccessRule {
    EnsureResource::new(FirewallAccessRuleResource::new(source))
}

/// Desired IP access rule. Natural key: zone + `mode` + `target` + `value`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureFirewallAccessRuleInput {
    /// Zone id (32-char hex). Provide this or [`zone_name`](Self::zone_name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_id: Option<String>,
    /// Zone DNS name (resolved via `GET /zones?name=…` when `zone_id` is absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_name: Option<String>,
    /// `block`, `challenge`, `whitelist`, `js_challenge`, `managed_challenge`.
    pub mode: String,
    /// Configuration target: `ip`, `ip6`, `ip_range`, `country`, `asn`.
    pub target: String,
    /// IP, CIDR, country code, or ASN value matching `target`.
    pub value: String,
    /// Optional notes (metadata only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Observed IP access rule — managed fields plus the Cloudflare rule id.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureFirewallAccessRuleOutput {
    pub zone_id: String,
    pub rule_id: String,
    pub mode: String,
    pub target: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Clone)]
pub struct FirewallAccessRuleResource {
    source: CloudflareClientSource,
}

impl FirewallAccessRuleResource {
    pub fn new(source: CloudflareClientSource) -> Self {
        Self { source }
    }

    async fn find(
        &self,
        ctx: &ResourceCtx,
        zone_id: &str,
        spec: &EnsureFirewallAccessRuleInput,
    ) -> ResourceResult<Option<AccessRule>> {
        let client = self.source.client(ctx).await?;
        let query = AccessRuleListQuery {
            mode: Some(spec.mode.clone()),
            configuration_target: Some(spec.target.clone()),
            configuration_value: Some(spec.value.clone()),
            per_page: Some(100),
            ..Default::default()
        };
        let rules = client
            .firewall_access_rules(zone_id, &query)
            .await
            .map_err(ResourceError::provider)?;
        Ok(rules.into_iter().find(|r| {
            r.mode == spec.mode
                && r.configuration.target == spec.target
                && r.configuration.value == spec.value
        }))
    }
}

fn to_output(zone_id: &str, rule: AccessRule) -> Option<EnsureFirewallAccessRuleOutput> {
    let id = rule.id?;
    Some(EnsureFirewallAccessRuleOutput {
        zone_id: zone_id.to_string(),
        rule_id: id,
        mode: rule.mode,
        target: rule.configuration.target,
        value: rule.configuration.value,
        notes: rule.notes,
    })
}

fn build(spec: &EnsureFirewallAccessRuleInput) -> AccessRule {
    AccessRule {
        mode: spec.mode.clone(),
        configuration: AccessRuleConfiguration {
            target: spec.target.clone(),
            value: spec.value.clone(),
        },
        notes: spec.notes.clone(),
        ..Default::default()
    }
}

#[async_trait]
impl Resource for FirewallAccessRuleResource {
    type Spec = EnsureFirewallAccessRuleInput;
    type State = EnsureFirewallAccessRuleOutput;

    fn kind(&self) -> &'static str {
        ENSURE_FIREWALL_ACCESS_RULE
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let zone_id = resolve_zone_id(&self.source, ctx, &spec.zone_id, &spec.zone_name).await?;
        Ok(self
            .find(ctx, &zone_id, spec)
            .await?
            .and_then(|r| to_output(&zone_id, r)))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let zone_id = resolve_zone_id(&self.source, ctx, &spec.zone_id, &spec.zone_name).await?;
        let client = self.source.client(ctx).await?;
        let created = client
            .create_firewall_access_rule(&zone_id, &build(spec))
            .await
            .map_err(ResourceError::provider)?;
        to_output(&zone_id, created)
            .ok_or_else(|| ResourceError::provider("created access rule has no id"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        if let Some(ref notes) = spec.notes {
            if current.notes.as_deref() != Some(notes.as_str()) {
                return Drift::Drifted(format!("notes {:?} → {:?}", current.notes, notes));
            }
        }
        Drift::InSync
    }

    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let zone_id = resolve_zone_id(&self.source, ctx, &spec.zone_id, &spec.zone_name).await?;
        let client = self.source.client(ctx).await?;
        let mut rule = self
            .find(ctx, &zone_id, spec)
            .await?
            .ok_or_else(|| ResourceError::provider("access rule disappeared before reconcile"))?;
        if spec.notes.is_some() {
            rule.notes = spec.notes.clone();
        }
        let updated = client
            .update_firewall_access_rule(&zone_id, &current.rule_id, &rule)
            .await
            .map_err(ResourceError::provider)?;
        to_output(&zone_id, updated)
            .ok_or_else(|| ResourceError::provider("updated access rule has no id"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> FirewallAccessRuleResource {
        FirewallAccessRuleResource::new(CloudflareClientSource::vault("cloud/cloudflare.vault"))
    }

    fn current() -> EnsureFirewallAccessRuleOutput {
        EnsureFirewallAccessRuleOutput {
            zone_id: "zone123".into(),
            rule_id: "rule456".into(),
            mode: "block".into(),
            target: "ip".into(),
            value: "198.51.100.4".into(),
            notes: Some("scanner".into()),
        }
    }

    #[test]
    fn matching_spec_is_in_sync() {
        let spec = EnsureFirewallAccessRuleInput {
            zone_id: Some("zone123".into()),
            mode: "block".into(),
            target: "ip".into(),
            value: "198.51.100.4".into(),
            notes: Some("scanner".into()),
            ..Default::default()
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_notes_drifts() {
        let spec = EnsureFirewallAccessRuleInput {
            zone_id: Some("zone123".into()),
            mode: "block".into(),
            target: "ip".into(),
            value: "198.51.100.4".into(),
            notes: Some("updated note".into()),
            ..Default::default()
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }
}
