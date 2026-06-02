//! Fluent infra builder extension for IONOS native nodes.

use crate::client::IonosClientSource;
use crate::methods::{
    ensure_datacenter, ensure_server, ensure_volume, EnsureDatacenter, EnsureDatacenterInput,
    EnsureServer, EnsureServerInput, EnsureVolume, EnsureVolumeInput,
};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_ext_ionos_cloud_api::IonosClient;

/// Extension trait: attach IONOS methods to an [`InfraBuilder`].
pub trait IonosInfraExt {
    /// Register IONOS methods against a ready client (e.g. from `client_from_env`).
    fn ionos(self, client: IonosClient, machine_id: MachineId) -> IonosInfraBuilder;

    /// Register IONOS methods that read the API token from the controller vault at
    /// apply time (no `IONOS_*` environment variables needed).
    ///
    /// `file` is the vault file (under `files/`) holding `token` and an optional
    /// `contract_number`; its DataKey must be among the run's unlocked keys. Use
    /// [`IonosClientSource::vault_fields`] for non-default field names.
    fn ionos_vault(self, file: impl Into<String>, machine_id: MachineId) -> IonosInfraBuilder;
}

impl IonosInfraExt for InfraBuilder {
    fn ionos(self, client: IonosClient, machine_id: MachineId) -> IonosInfraBuilder {
        IonosInfraBuilder::new(self, IonosClientSource::ready(client), machine_id)
    }

    fn ionos_vault(self, file: impl Into<String>, machine_id: MachineId) -> IonosInfraBuilder {
        IonosInfraBuilder::new(self, IonosClientSource::vault(file), machine_id)
    }
}

/// Staged builder with IONOS methods pre-registered.
pub struct IonosInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl IonosInfraBuilder {
    pub fn new(builder: InfraBuilder, source: IonosClientSource, machine_id: MachineId) -> Self {
        let builder = builder
            .method(ensure_datacenter(source.clone()))
            .method(ensure_server(source.clone()))
            .method(ensure_volume(source));
        Self {
            builder,
            machine_id,
        }
    }

    /// Ensure a data center exists (create or skip).
    pub fn ensure_datacenter(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureDatacenterInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureDatacenter>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Ensure a server exists in a data center (create or skip).
    ///
    /// Captured outputs (server id, vm state) are available for downstream nodes;
    /// the server is not registered as an infrazeug machine.
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

    /// Ensure a block volume exists in a data center (create or skip).
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

    /// Return the underlying [`InfraBuilder`] for further chaining.
    pub fn into_builder(self) -> InfraBuilder {
        self.builder
    }

    /// Finish as a [`PlaybookBundle`].
    pub fn finish(self) -> PlaybookBundle {
        self.builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_api::builder;
    use infrazeug_ext_ionos_cloud_api::{Auth, IonosConfig};
    use uuid::Uuid;

    fn dummy_client() -> IonosClient {
        IonosClient::new(IonosConfig::new(Auth::token("dummy")))
    }

    #[test]
    fn ionos_resources_plan() {
        let local = MachineId(Uuid::new_v4());
        let server = NodeId(Uuid::new_v4());
        let volume = NodeId(Uuid::new_v4());
        let bundle = InfraBuilder::new()
            .machine(builder::controller(local))
            .unwrap()
            .ionos(dummy_client(), local)
            .ensure_server(
                server,
                "web-1",
                EnsureServerInput {
                    datacenter_id: "dc-1".into(),
                    name: "web-1".into(),
                    cores: 2,
                    ram: 4096,
                    availability_zone: Some("AUTO".into()),
                    cpu_family: None,
                },
            )
            .unwrap()
            .ensure_volume(
                volume,
                "web-1-data",
                EnsureVolumeInput {
                    datacenter_id: "dc-1".into(),
                    name: "web-1-data".into(),
                    disk_type: "SSD".into(),
                    size: 50,
                    availability_zone: Some("AUTO".into()),
                    image: None,
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
