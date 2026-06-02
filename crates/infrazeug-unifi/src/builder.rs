//! Fluent infra builder extension for UniFi native nodes.

use crate::client::UnifiClientSource;
use crate::methods::{
    ensure_dns_record, ensure_firewall_group, ensure_firewall_rule, ensure_fixed_ip,
    ensure_network, ensure_port_forward, ensure_user_group, ensure_wlan, EnsureDnsRecord,
    EnsureDnsRecordInput, EnsureFirewallGroup, EnsureFirewallGroupInput, EnsureFirewallRule,
    EnsureFirewallRuleInput, EnsureFixedIp, EnsureFixedIpInput, EnsureNetwork, EnsureNetworkInput,
    EnsurePortForward, EnsurePortForwardInput, EnsureUserGroup, EnsureUserGroupInput, EnsureWlan,
    EnsureWlanInput,
};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_ext_unifi_api::UnifiClient;

/// Extension trait: attach UniFi methods to an [`InfraBuilder`].
pub trait UnifiInfraExt {
    /// Register UniFi methods bound to a ready [`UnifiClient`]. Configure TLS
    /// verification (and site / controller flavour) on the client's
    /// [`UnifiConfig`] — e.g. `UnifiConfig::new(..).insecure()` to ignore the
    /// controller's self-signed certificate.
    fn unifi(self, client: UnifiClient, machine_id: MachineId) -> UnifiInfraBuilder;

    /// Register UniFi methods with credentials read from the controller vault
    /// `file` at apply time: an `api_key` field (key auth) if present, otherwise
    /// `username` / `password` (session login). `host` is non-secret config; TLS
    /// verification is on. To ignore the controller's certificate, target a
    /// different site, or use a legacy controller, build the source explicitly and
    /// pass it to [`unifi_source`](Self::unifi_source):
    ///
    /// ```ignore
    /// builder.unifi_source(
    ///     UnifiClientSource::vault(host, "cloud/unifi.vault").insecure(),
    ///     machine_id,
    /// )
    /// ```
    fn unifi_vault(
        self,
        host: impl Into<String>,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> UnifiInfraBuilder;

    /// Register UniFi methods bound to a pre-built [`UnifiClientSource`] — the
    /// escape hatch for full control over TLS verification, site, controller
    /// flavour, and credential fields.
    fn unifi_source(self, source: UnifiClientSource, machine_id: MachineId) -> UnifiInfraBuilder;
}

impl UnifiInfraExt for InfraBuilder {
    fn unifi(self, client: UnifiClient, machine_id: MachineId) -> UnifiInfraBuilder {
        UnifiInfraBuilder::new(self, UnifiClientSource::ready(client), machine_id)
    }

    fn unifi_vault(
        self,
        host: impl Into<String>,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> UnifiInfraBuilder {
        UnifiInfraBuilder::new(self, UnifiClientSource::vault(host, file), machine_id)
    }

    fn unifi_source(self, source: UnifiClientSource, machine_id: MachineId) -> UnifiInfraBuilder {
        UnifiInfraBuilder::new(self, source, machine_id)
    }
}

/// Staged builder with UniFi methods pre-registered.
pub struct UnifiInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl UnifiInfraBuilder {
    pub fn new(builder: InfraBuilder, source: UnifiClientSource, machine_id: MachineId) -> Self {
        let builder = builder
            .method(ensure_network(source.clone()))
            .method(ensure_port_forward(source.clone()))
            .method(ensure_wlan(source.clone()))
            .method(ensure_dns_record(source.clone()))
            .method(ensure_firewall_group(source.clone()))
            .method(ensure_firewall_rule(source.clone()))
            .method(ensure_user_group(source.clone()))
            .method(ensure_fixed_ip(source));
        Self {
            builder,
            machine_id,
        }
    }

    /// Build a client source from a ready client, exposing the underlying methods
    /// without going through the staged builder (e.g. for custom wiring).
    pub fn source(client: UnifiClient) -> UnifiClientSource {
        UnifiClientSource::ready(client)
    }

    pub fn ensure_network(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureNetworkInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureNetwork>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn ensure_port_forward(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsurePortForwardInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsurePortForward>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn ensure_wlan(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureWlanInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureWlan>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

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

    pub fn ensure_firewall_group(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureFirewallGroupInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureFirewallGroup>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn ensure_firewall_rule(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureFirewallRuleInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureFirewallRule>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn ensure_user_group(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureUserGroupInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureUserGroup>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn ensure_fixed_ip(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureFixedIpInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureFixedIp>(node_id, name, self.machine_id, input)?
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
    use infrazeug_ext_unifi_api::{Credentials, UnifiConfig};
    use uuid::Uuid;

    fn dummy_client() -> UnifiClient {
        UnifiClient::new(UnifiConfig::new(
            "https://unifi.local",
            Credentials::user_pass("admin", "dummy"),
        ))
    }

    #[test]
    fn unifi_resources_plan() {
        let local = MachineId(Uuid::new_v4());
        let network = NodeId(Uuid::new_v4());
        let wlan = NodeId(Uuid::new_v4());
        let port_forward = NodeId(Uuid::new_v4());
        let dns = NodeId(Uuid::new_v4());
        let fw_group = NodeId(Uuid::new_v4());
        let fw_rule = NodeId(Uuid::new_v4());
        let user_group = NodeId(Uuid::new_v4());
        let fixed_ip = NodeId(Uuid::new_v4());
        let bundle = InfraBuilder::new()
            .machine(builder::controller(local))
            .unwrap()
            .unifi(dummy_client(), local)
            .ensure_network(
                network,
                "iot",
                EnsureNetworkInput {
                    name: "iot".into(),
                    purpose: "vlan-only".into(),
                    vlan: Some(20),
                    ..Default::default()
                },
            )
            .unwrap()
            .ensure_wlan(
                wlan,
                "iot-ssid",
                EnsureWlanInput {
                    name: "iot".into(),
                    security: "wpapsk".into(),
                    passphrase: Some("supersecret".into()),
                    ..Default::default()
                },
            )
            .unwrap()
            .ensure_port_forward(
                port_forward,
                "web",
                EnsurePortForwardInput {
                    name: "web".into(),
                    fwd: "10.0.0.10".into(),
                    fwd_port: "80".into(),
                    dst_port: "80".into(),
                    proto: "tcp".into(),
                    pfwd_interface: "wan".into(),
                    src: "any".into(),
                    enabled: Some(true),
                },
            )
            .unwrap()
            .ensure_dns_record(
                dns,
                "nas-dns",
                EnsureDnsRecordInput {
                    name: "nas.lan".into(),
                    record_type: "A".into(),
                    value: "10.0.0.5".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .ensure_firewall_group(
                fw_group,
                "admins",
                EnsureFirewallGroupInput {
                    name: "admins".into(),
                    group_type: "address-group".into(),
                    members: vec!["10.0.0.2".into()],
                },
            )
            .unwrap()
            .ensure_firewall_rule(
                fw_rule,
                "block-iot",
                EnsureFirewallRuleInput {
                    name: "block-iot".into(),
                    ruleset: "LAN_IN".into(),
                    rule_index: 2000,
                    action: "drop".into(),
                    protocol: "all".into(),
                    enabled: Some(true),
                    ..Default::default()
                },
            )
            .unwrap()
            .ensure_user_group(
                user_group,
                "throttled",
                EnsureUserGroupInput {
                    name: "throttled".into(),
                    qos_rate_max_down: 50_000,
                    qos_rate_max_up: 10_000,
                },
            )
            .unwrap()
            .ensure_fixed_ip(
                fixed_ip,
                "printer",
                EnsureFixedIpInput {
                    mac: "aa:bb:cc:dd:ee:ff".into(),
                    fixed_ip: "10.0.0.50".into(),
                    network_id: "net-1".into(),
                    name: Some("printer".into()),
                    ..Default::default()
                },
            )
            .unwrap()
            .finish();

        // `build()` injects a per-machine connectivity head plus the global
        // begin/finish bookends; count the real (user-authored) nodes.
        let real_nodes = bundle
            .infra
            .nodes
            .iter()
            .filter(|n| !(n.body.is_group_bookend() || n.body.is_connect()))
            .count();
        assert_eq!(real_nodes, 8);
        bundle.plan().expect("lint + plan");
    }
}
