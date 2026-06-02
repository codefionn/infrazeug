//! Fluent infra builder extension for STACKIT native nodes.

use crate::client::StackitClientSource;
use crate::methods::{
    ensure_server, ensure_volume, EnsureServer, EnsureServerInput, EnsureVolume, EnsureVolumeInput,
};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_ext_stackit_api::StackitClient;

/// Extension trait: attach STACKIT methods to an [`InfraBuilder`].
pub trait StackitInfraExt {
    fn stackit(self, client: StackitClient, machine_id: MachineId) -> StackitInfraBuilder;

    /// Register STACKIT methods that read credentials from the controller vault at
    /// apply time (`token` or `service_account_key` + optional `private_key`).
    fn stackit_vault(self, file: impl Into<String>, machine_id: MachineId) -> StackitInfraBuilder;
}

impl StackitInfraExt for InfraBuilder {
    fn stackit(self, client: StackitClient, machine_id: MachineId) -> StackitInfraBuilder {
        StackitInfraBuilder::new(self, StackitClientSource::ready(client), machine_id)
    }

    fn stackit_vault(self, file: impl Into<String>, machine_id: MachineId) -> StackitInfraBuilder {
        StackitInfraBuilder::new(self, StackitClientSource::vault(file), machine_id)
    }
}

/// Staged builder with STACKIT methods pre-registered.
pub struct StackitInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl StackitInfraBuilder {
    pub fn new(builder: InfraBuilder, source: StackitClientSource, machine_id: MachineId) -> Self {
        let builder = builder
            .method(ensure_server(source.clone()))
            .method(ensure_volume(source));
        Self {
            builder,
            machine_id,
        }
    }

    pub fn ensure_server(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureServerInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureServer>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn ensure_volume(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureVolumeInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureVolume>(node_id, name, self.machine_id, input)?
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
    use infrazeug_ext_stackit_api::{Auth, StackitConfig};
    use uuid::Uuid;

    fn dummy_client() -> StackitClient {
        StackitClient::new(StackitConfig::new(Auth::token("dummy")))
    }

    #[test]
    fn stackit_resources_plan() {
        let local = MachineId(Uuid::new_v4());
        let server = NodeId(Uuid::new_v4());
        let volume = NodeId(Uuid::new_v4());
        let bundle = InfraBuilder::new()
            .machine(builder::controller(local))
            .unwrap()
            .stackit(dummy_client(), local)
            .ensure_volume(
                volume,
                "boot",
                EnsureVolumeInput {
                    project_id: "proj-1".into(),
                    name: "boot".into(),
                    size: 10,
                    availability_zone: Some("eu01-1".into()),
                    source_id: Some("image-1".into()),
                    source_type: "image".into(),
                    performance_class: None,
                    region: None,
                },
            )
            .unwrap()
            .ensure_server(
                server,
                "web-1",
                EnsureServerInput {
                    project_id: "proj-1".into(),
                    name: "web-1".into(),
                    machine_type: "g2i.1".into(),
                    boot_volume_id: "vol-1".into(),
                    boot_volume_source_type: "volume".into(),
                    availability_zone: None,
                    keypair_name: None,
                    network_id: None,
                    security_groups: None,
                    region: None,
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
