use crate::client::AzureClientSource;
use crate::methods::{
    ensure_container, ensure_disk, ensure_storage_key, ensure_vm, EnsureContainer,
    EnsureContainerInput, EnsureDisk, EnsureDiskInput, EnsureStorageKey, EnsureStorageKeyInput,
    EnsureVm, EnsureVmInput,
};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_ext_azure_api::AzureClient;

pub trait AzureInfraExt {
    fn azure(self, client: AzureClient, machine_id: MachineId) -> AzureInfraBuilder;
    fn azure_vault(self, file: impl Into<String>, machine_id: MachineId) -> AzureInfraBuilder;
}

impl AzureInfraExt for InfraBuilder {
    fn azure(self, client: AzureClient, machine_id: MachineId) -> AzureInfraBuilder {
        AzureInfraBuilder::new(self, AzureClientSource::ready(client), machine_id)
    }

    fn azure_vault(self, file: impl Into<String>, machine_id: MachineId) -> AzureInfraBuilder {
        AzureInfraBuilder::new(self, AzureClientSource::vault(file), machine_id)
    }
}

pub struct AzureInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl AzureInfraBuilder {
    pub fn new(builder: InfraBuilder, source: AzureClientSource, machine_id: MachineId) -> Self {
        let builder = builder
            .method(ensure_vm(source.clone()))
            .method(ensure_disk(source.clone()))
            .method(ensure_container(source.clone()))
            .method(ensure_storage_key(source));
        Self {
            builder,
            machine_id,
        }
    }

    pub fn ensure_vm(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureVmInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureVm>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn ensure_disk(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureDiskInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureDisk>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn ensure_container(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureContainerInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureContainer>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn ensure_storage_key(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureStorageKeyInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureStorageKey>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn finish(self) -> PlaybookBundle {
        self.builder.build()
    }
}
