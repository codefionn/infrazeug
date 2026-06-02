use crate::client::GcpClientSource;
use crate::methods::{
    ensure_bucket, ensure_disk, ensure_instance, ensure_service_account_key, EnsureBucket,
    EnsureBucketInput, EnsureDisk, EnsureDiskInput, EnsureInstance, EnsureInstanceInput,
    EnsureServiceAccountKey, EnsureServiceAccountKeyInput,
};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_ext_gcp_api::GcpClient;

pub trait GcpInfraExt {
    fn gcp(self, client: GcpClient, machine_id: MachineId) -> GcpInfraBuilder;
    fn gcp_vault(self, file: impl Into<String>, machine_id: MachineId) -> GcpInfraBuilder;
}

impl GcpInfraExt for InfraBuilder {
    fn gcp(self, client: GcpClient, machine_id: MachineId) -> GcpInfraBuilder {
        GcpInfraBuilder::new(self, GcpClientSource::ready(client), machine_id)
    }

    fn gcp_vault(self, file: impl Into<String>, machine_id: MachineId) -> GcpInfraBuilder {
        GcpInfraBuilder::new(self, GcpClientSource::vault(file), machine_id)
    }
}

pub struct GcpInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl GcpInfraBuilder {
    pub fn new(builder: InfraBuilder, source: GcpClientSource, machine_id: MachineId) -> Self {
        let builder = builder
            .method(ensure_instance(source.clone()))
            .method(ensure_disk(source.clone()))
            .method(ensure_bucket(source.clone()))
            .method(ensure_service_account_key(source));
        Self {
            builder,
            machine_id,
        }
    }

    pub fn ensure_instance(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureInstanceInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureInstance>(node_id, name, self.machine_id, input)?
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

    pub fn ensure_bucket(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureBucketInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureBucket>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn ensure_service_account_key(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureServiceAccountKeyInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureServiceAccountKey>(node_id, name, self.machine_id, input)?
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
