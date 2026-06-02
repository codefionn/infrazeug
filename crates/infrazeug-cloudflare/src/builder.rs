//! Fluent infra builder extension for Cloudflare native nodes.

use crate::client::CloudflareClientSource;
use crate::methods::{
    ensure_dns_record, ensure_firewall_access_rule, ensure_kv_namespace, ensure_r2_bucket,
    ensure_waf_custom_rule, ensure_zone_setting, EnsureDnsRecord, EnsureDnsRecordInput,
    EnsureFirewallAccessRule, EnsureFirewallAccessRuleInput, EnsureKvNamespace,
    EnsureKvNamespaceInput, EnsureR2Bucket, EnsureR2BucketInput, EnsureWafCustomRule,
    EnsureWafCustomRuleInput, EnsureZoneSetting, EnsureZoneSettingInput,
};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_ext_cloudflare_api::CloudflareClient;

/// Extension trait: attach Cloudflare methods to an [`InfraBuilder`].
pub trait CloudflareInfraExt {
    /// Register Cloudflare methods bound to a ready [`CloudflareClient`].
    fn cloudflare(self, client: CloudflareClient, machine_id: MachineId) -> CloudflareInfraBuilder;

    /// Register Cloudflare methods with credentials read from the controller vault
    /// `file` at apply time. An `api_token` field is tried first; if absent,
    /// `email` / `api_key` are used for global-key auth.
    fn cloudflare_vault(
        self,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> CloudflareInfraBuilder;

    /// Register Cloudflare methods bound to a pre-built [`CloudflareClientSource`].
    fn cloudflare_source(
        self,
        source: CloudflareClientSource,
        machine_id: MachineId,
    ) -> CloudflareInfraBuilder;
}

impl CloudflareInfraExt for InfraBuilder {
    fn cloudflare(self, client: CloudflareClient, machine_id: MachineId) -> CloudflareInfraBuilder {
        CloudflareInfraBuilder::new(self, CloudflareClientSource::ready(client), machine_id)
    }

    fn cloudflare_vault(
        self,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> CloudflareInfraBuilder {
        CloudflareInfraBuilder::new(self, CloudflareClientSource::vault(file), machine_id)
    }

    fn cloudflare_source(
        self,
        source: CloudflareClientSource,
        machine_id: MachineId,
    ) -> CloudflareInfraBuilder {
        CloudflareInfraBuilder::new(self, source, machine_id)
    }
}

/// Staged builder with Cloudflare methods pre-registered.
pub struct CloudflareInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl CloudflareInfraBuilder {
    pub fn new(
        builder: InfraBuilder,
        source: CloudflareClientSource,
        machine_id: MachineId,
    ) -> Self {
        let builder = builder
            .method(ensure_dns_record(source.clone()))
            .method(ensure_zone_setting(source.clone()))
            .method(ensure_firewall_access_rule(source.clone()))
            .method(ensure_waf_custom_rule(source.clone()))
            .method(ensure_r2_bucket(source.clone()))
            .method(ensure_kv_namespace(source));
        Self {
            builder,
            machine_id,
        }
    }

    /// Ensure a DNS record exists in a zone (create or reconcile on drift).
    pub fn ensure_dns_record(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureDnsRecordInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureDnsRecord>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Ensure a zone setting value (SSL mode, always_use_https, …).
    pub fn ensure_zone_setting(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureZoneSettingInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureZoneSetting>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Ensure a zone IP access rule (block/allow/challenge by IP, CIDR, country, …).
    pub fn ensure_firewall_access_rule(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureFirewallAccessRuleInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureFirewallAccessRule>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Ensure a custom WAF rule in the managed zone ruleset.
    pub fn ensure_waf_custom_rule(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureWafCustomRuleInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureWafCustomRule>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Ensure an R2 object-storage bucket exists (create or reconcile storage class).
    pub fn ensure_r2_bucket(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureR2BucketInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureR2Bucket>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Ensure a Workers KV namespace exists (create-only; title is immutable).
    pub fn ensure_kv_namespace(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureKvNamespaceInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureKvNamespace>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn into_builder(self) -> InfraBuilder {
        self.builder
    }

    pub fn finish(self) -> PlaybookBundle {
        self.builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_api::builder;
    use infrazeug_ext_cloudflare_api::{Auth, CloudflareClient, CloudflareConfig};
    use uuid::Uuid;

    fn dummy_client() -> CloudflareClient {
        CloudflareClient::new(CloudflareConfig::new(Auth::token("dummy")))
    }

    #[test]
    fn ensure_dns_record_plans() {
        let local = MachineId(Uuid::new_v4());
        let node = NodeId(Uuid::new_v4());
        let bundle = InfraBuilder::new()
            .machine(builder::controller(local))
            .unwrap()
            .cloudflare(dummy_client(), local)
            .ensure_dns_record(
                node,
                "www-a",
                EnsureDnsRecordInput {
                    zone_name: Some("example.com".into()),
                    name: "www.example.com".into(),
                    record_type: "A".into(),
                    content: "192.0.2.1".into(),
                    proxied: Some(true),
                    ..Default::default()
                },
            )
            .unwrap()
            .finish();

        bundle.plan().expect("lint + plan");
    }
}
