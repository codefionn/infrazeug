//! Fluent infra builder extension for MikroTik native nodes.

use crate::client::{MikrotikClientSource, MikrotikParams};
use crate::methods::{
    ensure_firewall_rule, ensure_ip_address, EnsureFirewallRule, EnsureFirewallRuleInput,
    EnsureIpAddress, EnsureIpAddressInput,
};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};

/// Extension trait: attach MikroTik methods to an [`InfraBuilder`].
pub trait MikrotikInfraExt {
    /// Register MikroTik methods bound to ready connection parameters. Configure
    /// TLS verification on [`MikrotikParams`] — e.g.
    /// `MikrotikConfig::new(host).with_tls(true).insecure()` for API-SSL with a
    /// stock self-signed certificate, or [`MikrotikParams::insecure`] after
    /// [`with_tls`](MikrotikParams::with_tls).
    fn mikrotik(self, params: MikrotikParams, machine_id: MachineId) -> MikrotikInfraBuilder;

    /// Register MikroTik methods with credentials read from the controller vault
    /// `file` at apply time. `host` is non-secret config; TLS verification is on.
    /// To ignore the router's API-SSL certificate or select API-SSL, build the
    /// source explicitly and pass it to [`mikrotik_source`](Self::mikrotik_source):
    ///
    /// ```ignore
    /// builder.mikrotik_source(
    ///     MikrotikClientSource::vault(host, "cloud/mikrotik.vault")
    ///         .with_tls(true)
    ///         .insecure(),
    ///     machine_id,
    /// )
    /// ```
    fn mikrotik_vault(
        self,
        host: impl Into<String>,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> MikrotikInfraBuilder;

    /// Register MikroTik methods bound to a pre-built [`MikrotikClientSource`] —
    /// the escape hatch for full control over TLS, port, and credential fields.
    fn mikrotik_source(
        self,
        source: MikrotikClientSource,
        machine_id: MachineId,
    ) -> MikrotikInfraBuilder;
}

impl MikrotikInfraExt for InfraBuilder {
    fn mikrotik(self, params: MikrotikParams, machine_id: MachineId) -> MikrotikInfraBuilder {
        MikrotikInfraBuilder::new(self, MikrotikClientSource::ready(params), machine_id)
    }

    fn mikrotik_vault(
        self,
        host: impl Into<String>,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> MikrotikInfraBuilder {
        MikrotikInfraBuilder::new(self, MikrotikClientSource::vault(host, file), machine_id)
    }

    fn mikrotik_source(
        self,
        source: MikrotikClientSource,
        machine_id: MachineId,
    ) -> MikrotikInfraBuilder {
        MikrotikInfraBuilder::new(self, source, machine_id)
    }
}

/// Staged builder with MikroTik methods pre-registered.
pub struct MikrotikInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl MikrotikInfraBuilder {
    pub fn new(builder: InfraBuilder, source: MikrotikClientSource, machine_id: MachineId) -> Self {
        let builder = builder
            .method(ensure_ip_address(source.clone()))
            .method(ensure_firewall_rule(source));
        Self {
            builder,
            machine_id,
        }
    }

    pub fn ensure_ip_address(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureIpAddressInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureIpAddress>(node_id, name, self.machine_id, input)?
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
    use infrazeug_ext_mikrotik_api::{Credentials, MikrotikConfig};
    use uuid::Uuid;

    fn dummy_params() -> MikrotikParams {
        MikrotikParams {
            config: MikrotikConfig::new("192.168.88.1"),
            credentials: Credentials::new("admin", "dummy"),
        }
    }

    #[test]
    fn mikrotik_resources_plan() {
        let local = MachineId(Uuid::new_v4());
        let addr = NodeId(Uuid::new_v4());
        let fw = NodeId(Uuid::new_v4());
        let bundle = InfraBuilder::new()
            .machine(builder::controller(local))
            .unwrap()
            .mikrotik(dummy_params(), local)
            .ensure_ip_address(
                addr,
                "mgmt",
                EnsureIpAddressInput {
                    address: "192.168.88.2/24".into(),
                    interface: "bridge".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .ensure_firewall_rule(
                fw,
                "allow-ssh",
                EnsureFirewallRuleInput {
                    comment: "allow-ssh".into(),
                    chain: "input".into(),
                    action: "accept".into(),
                    protocol: "tcp".into(),
                    dst_port: Some("22".into()),
                    ..Default::default()
                },
            )
            .unwrap()
            .finish();

        bundle.plan().expect("lint + plan");
    }
}
