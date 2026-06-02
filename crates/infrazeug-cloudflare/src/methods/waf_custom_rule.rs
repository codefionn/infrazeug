//! Ensure a custom WAF rule exists in a managed zone ruleset.

use crate::client::CloudflareClientSource;
use crate::methods::zone::resolve_zone_id;
use async_trait::async_trait;
use infrazeug_ext_cloudflare_api::ruleset::{phase, Ruleset, RulesetCreate, RulesetRule};
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_WAF_CUSTOM_RULE: &str = "cloudflare.ensure_waf_custom_rule";

const DEFAULT_RULESET_NAME: &str = "infrazeug-managed-waf";

/// Tier-1 method: ensure a custom WAF rule in a zone ruleset.
pub type EnsureWafCustomRule = EnsureResource<WafCustomRuleResource>;

/// Construct the registrable [`EnsureWafCustomRule`] method for a client source.
pub fn ensure_waf_custom_rule(source: CloudflareClientSource) -> EnsureWafCustomRule {
    EnsureResource::new(WafCustomRuleResource::new(source))
}

fn default_enabled() -> bool {
    true
}

/// Desired custom WAF rule. Natural key: zone + `description`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EnsureWafCustomRuleInput {
    /// Zone id (32-char hex). Provide this or [`zone_name`](Self::zone_name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_id: Option<String>,
    /// Zone DNS name (resolved via `GET /zones?name=…` when `zone_id` is absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_name: Option<String>,
    /// Human-readable rule label (unique within the managed ruleset).
    pub description: String,
    /// Cloudflare Rules expression.
    pub expression: String,
    /// `block`, `challenge`, `js_challenge`, `managed_challenge`, `log`, `bypass`, …
    pub action: String,
    /// Optional stable `ref` tag for the rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// Managed custom ruleset name (default `infrazeug-managed-waf`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruleset_name: Option<String>,
    /// Whether the rule is enabled (default `true`).
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl Default for EnsureWafCustomRuleInput {
    fn default() -> Self {
        Self {
            zone_id: None,
            zone_name: None,
            description: String::new(),
            expression: String::new(),
            action: String::new(),
            reference: None,
            ruleset_name: None,
            enabled: default_enabled(),
        }
    }
}

/// Observed custom WAF rule — managed fields plus ids.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureWafCustomRuleOutput {
    pub zone_id: String,
    pub ruleset_id: String,
    pub rule_id: String,
    pub description: String,
    pub expression: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct WafCustomRuleResource {
    source: CloudflareClientSource,
}

impl WafCustomRuleResource {
    pub fn new(source: CloudflareClientSource) -> Self {
        Self { source }
    }

    fn ruleset_name(spec: &EnsureWafCustomRuleInput) -> &str {
        spec.ruleset_name.as_deref().unwrap_or(DEFAULT_RULESET_NAME)
    }

    async fn find_managed_ruleset(
        &self,
        ctx: &ResourceCtx,
        zone_id: &str,
        spec: &EnsureWafCustomRuleInput,
    ) -> ResourceResult<Option<Ruleset>> {
        let client = self.source.client(ctx).await?;
        let name = Self::ruleset_name(spec);
        let rulesets = client
            .rulesets(zone_id)
            .await
            .map_err(ResourceError::provider)?;
        Ok(rulesets.into_iter().find(|r| {
            r.kind.as_deref() == Some("zone")
                && r.phase.as_deref() == Some(phase::HTTP_REQUEST_FIREWALL_CUSTOM)
                && r.name.as_deref() == Some(name)
        }))
    }

    async fn find_rule(
        &self,
        ctx: &ResourceCtx,
        zone_id: &str,
        spec: &EnsureWafCustomRuleInput,
    ) -> ResourceResult<Option<(Ruleset, RulesetRule)>> {
        let Some(ruleset) = self.find_managed_ruleset(ctx, zone_id, spec).await? else {
            return Ok(None);
        };
        let ruleset_id = ruleset
            .id
            .clone()
            .ok_or_else(|| ResourceError::provider("managed WAF ruleset has no id"))?;
        let client = self.source.client(ctx).await?;
        let full = client
            .ruleset(zone_id, &ruleset_id)
            .await
            .map_err(ResourceError::provider)?;
        let rule = full
            .rules
            .as_ref()
            .unwrap_or(&Vec::new())
            .iter()
            .find(|r| r.description.as_deref() == Some(spec.description.as_str()))
            .cloned();
        Ok(rule.map(|r| (full, r)))
    }
}

fn to_output(
    zone_id: &str,
    ruleset_id: &str,
    rule: RulesetRule,
) -> Option<EnsureWafCustomRuleOutput> {
    let reference = rule.reference().map(str::to_string);
    Some(EnsureWafCustomRuleOutput {
        zone_id: zone_id.to_string(),
        ruleset_id: ruleset_id.to_string(),
        rule_id: rule.id?,
        description: rule.description?,
        expression: rule.expression?,
        action: rule.action?,
        reference,
        enabled: rule.enabled.unwrap_or(true),
    })
}

fn build_rule(spec: &EnsureWafCustomRuleInput) -> RulesetRule {
    RulesetRule {
        description: Some(spec.description.clone()),
        expression: Some(spec.expression.clone()),
        action: Some(spec.action.clone()),
        ref_: spec.reference.clone(),
        enabled: Some(spec.enabled),
        ..Default::default()
    }
}

#[async_trait]
impl Resource for WafCustomRuleResource {
    type Spec = EnsureWafCustomRuleInput;
    type State = EnsureWafCustomRuleOutput;

    fn kind(&self) -> &'static str {
        ENSURE_WAF_CUSTOM_RULE
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let zone_id = resolve_zone_id(&self.source, ctx, &spec.zone_id, &spec.zone_name).await?;
        Ok(self
            .find_rule(ctx, &zone_id, spec)
            .await?
            .and_then(|(ruleset, rule)| to_output(&zone_id, ruleset.id.as_deref()?, rule)))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let zone_id = resolve_zone_id(&self.source, ctx, &spec.zone_id, &spec.zone_name).await?;
        let client = self.source.client(ctx).await?;
        let ruleset_id =
            if let Some(ruleset) = self.find_managed_ruleset(ctx, &zone_id, spec).await? {
                ruleset
                    .id
                    .ok_or_else(|| ResourceError::provider("managed WAF ruleset has no id"))?
            } else {
                let created = client
                    .create_ruleset(
                        &zone_id,
                        &RulesetCreate {
                            name: Self::ruleset_name(spec).into(),
                            description: Some("infrazeug-managed custom WAF rules".into()),
                            kind: "zone".into(),
                            phase: phase::HTTP_REQUEST_FIREWALL_CUSTOM.into(),
                            rules: None,
                        },
                    )
                    .await
                    .map_err(ResourceError::provider)?;
                created
                    .id
                    .ok_or_else(|| ResourceError::provider("created WAF ruleset has no id"))?
            };
        let created_rule = client
            .create_ruleset_rule(&zone_id, &ruleset_id, &build_rule(spec))
            .await
            .map_err(ResourceError::provider)?;
        to_output(&zone_id, &ruleset_id, created_rule)
            .ok_or_else(|| ResourceError::provider("created WAF rule has no id"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if current.expression != spec.expression {
            diffs.push(format!(
                "expression {:?} → {:?}",
                current.expression, spec.expression
            ));
        }
        if current.action != spec.action {
            diffs.push(format!("action {:?} → {:?}", current.action, spec.action));
        }
        if current.enabled != spec.enabled {
            diffs.push(format!("enabled {} → {}", current.enabled, spec.enabled));
        }
        if let Some(ref reference) = spec.reference {
            if current.reference.as_deref() != Some(reference.as_str()) {
                diffs.push(format!(
                    "reference {:?} → {:?}",
                    current.reference, reference
                ));
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
        let zone_id = resolve_zone_id(&self.source, ctx, &spec.zone_id, &spec.zone_name).await?;
        let client = self.source.client(ctx).await?;
        let updated = client
            .update_ruleset_rule(
                &zone_id,
                &current.ruleset_id,
                &current.rule_id,
                &build_rule(spec),
            )
            .await
            .map_err(ResourceError::provider)?;
        to_output(&zone_id, &current.ruleset_id, updated)
            .ok_or_else(|| ResourceError::provider("updated WAF rule has no id"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> WafCustomRuleResource {
        WafCustomRuleResource::new(CloudflareClientSource::vault("cloud/cloudflare.vault"))
    }

    fn current() -> EnsureWafCustomRuleOutput {
        EnsureWafCustomRuleOutput {
            zone_id: "zone123".into(),
            ruleset_id: "rs456".into(),
            rule_id: "rule789".into(),
            description: "block admin".into(),
            expression: "(http.request.uri.path eq \"/admin\")".into(),
            action: "block".into(),
            reference: None,
            enabled: true,
        }
    }

    #[test]
    fn matching_spec_is_in_sync() {
        let spec = EnsureWafCustomRuleInput {
            zone_id: Some("zone123".into()),
            description: "block admin".into(),
            expression: "(http.request.uri.path eq \"/admin\")".into(),
            action: "block".into(),
            ..Default::default()
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_action_drifts() {
        let spec = EnsureWafCustomRuleInput {
            zone_id: Some("zone123".into()),
            description: "block admin".into(),
            expression: "(http.request.uri.path eq \"/admin\")".into(),
            action: "managed_challenge".into(),
            ..Default::default()
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }
}
