//! Fluent infra builder extension for Proxmox native nodes.

use crate::client::ProxmoxClientSource;
use crate::methods::{
    ensure_lxc, ensure_qemu, EnsureLxc, EnsureLxcInput, EnsureQemu, EnsureQemuInput,
};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_ext_proxmox_api::ProxmoxClient;

/// Extension trait: attach Proxmox methods to an [`InfraBuilder`].
pub trait ProxmoxInfraExt {
    fn proxmox(self, client: ProxmoxClient, machine_id: MachineId) -> ProxmoxInfraBuilder;

    /// Register Proxmox methods that read credentials from the controller vault at
    /// apply time (`host` + `token_id`/`token_secret` or `username`/`password`).
    fn proxmox_vault(self, file: impl Into<String>, machine_id: MachineId) -> ProxmoxInfraBuilder;
}

impl ProxmoxInfraExt for InfraBuilder {
    fn proxmox(self, client: ProxmoxClient, machine_id: MachineId) -> ProxmoxInfraBuilder {
        ProxmoxInfraBuilder::new(self, ProxmoxClientSource::ready(client), machine_id)
    }

    fn proxmox_vault(self, file: impl Into<String>, machine_id: MachineId) -> ProxmoxInfraBuilder {
        ProxmoxInfraBuilder::new(self, ProxmoxClientSource::vault(file), machine_id)
    }
}

/// Staged builder with Proxmox methods pre-registered.
pub struct ProxmoxInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl ProxmoxInfraBuilder {
    pub fn new(builder: InfraBuilder, source: ProxmoxClientSource, machine_id: MachineId) -> Self {
        let builder = builder
            .method(ensure_qemu(source.clone()))
            .method(ensure_lxc(source));
        Self {
            builder,
            machine_id,
        }
    }

    pub fn ensure_qemu(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureQemuInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureQemu>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn ensure_lxc(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureLxcInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureLxc>(node_id, name, self.machine_id, input)?
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
    use infrazeug_ext_proxmox_api::{Auth, ProxmoxConfig};
    use uuid::Uuid;

    fn dummy_client() -> ProxmoxClient {
        ProxmoxClient::new(ProxmoxConfig::new(
            "https://pve.example.com:8006",
            Auth::api_token("root@pam!ci", "dummy"),
        ))
    }

    #[test]
    fn proxmox_resources_plan() {
        let local = MachineId(Uuid::new_v4());
        let vm = NodeId(Uuid::new_v4());
        let ct = NodeId(Uuid::new_v4());
        let bundle = InfraBuilder::new()
            .machine(builder::controller(local))
            .unwrap()
            .proxmox(dummy_client(), local)
            .ensure_qemu(
                vm,
                "web-1",
                EnsureQemuInput {
                    node: "pve".into(),
                    vmid: 100,
                    name: Some("web-1".into()),
                    cores: Some(2),
                    memory: Some(2048),
                    net0: Some("virtio,bridge=vmbr0".into()),
                    ..Default::default()
                },
            )
            .unwrap()
            .ensure_lxc(
                ct,
                "ct-1",
                EnsureLxcInput {
                    node: "pve".into(),
                    vmid: 200,
                    ostemplate: "local:vztmpl/debian-12.tar.zst".into(),
                    hostname: Some("ct-1".into()),
                    cores: Some(1),
                    memory: Some(512),
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
        assert_eq!(real_nodes, 2);
        bundle.plan().expect("lint + plan");
    }
}
