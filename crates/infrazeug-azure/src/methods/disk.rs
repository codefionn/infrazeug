use crate::client::AzureClientSource;
use async_trait::async_trait;
use infrazeug_ext_azure_api::compute::{ManagedDisk, ManagedDiskCreate};
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_DISK: &str = "azure.ensure_disk";

pub type EnsureDisk = EnsureResource<DiskResource>;

pub fn ensure_disk(source: AzureClientSource) -> EnsureDisk {
    EnsureResource::new(DiskResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureDiskInput {
    pub name: String,
    pub resource_group: String,
    pub location: String,
    pub size_gb: u32,
    pub sku: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureDiskOutput {
    pub disk_id: String,
    pub name: String,
    pub resource_group: String,
    pub size_gb: u32,
}

#[derive(Clone)]
pub struct DiskResource {
    source: AzureClientSource,
}

impl DiskResource {
    pub fn new(source: AzureClientSource) -> Self {
        Self { source }
    }
}

fn to_output(disk: ManagedDisk) -> EnsureDiskOutput {
    EnsureDiskOutput {
        disk_id: disk.disk_id,
        name: disk.name,
        resource_group: disk.resource_group,
        size_gb: disk.size_gb,
    }
}

#[async_trait]
impl Resource for DiskResource {
    type Spec = EnsureDiskInput;
    type State = EnsureDiskOutput;

    fn kind(&self) -> &'static str {
        ENSURE_DISK
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let disks = client
            .compute_managed_disks(&spec.resource_group)
            .await
            .map_err(ResourceError::provider)?;
        Ok(disks
            .into_iter()
            .find(|d| d.name == spec.name)
            .map(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let created = client
            .compute_managed_disk_create(&ManagedDiskCreate {
                name: spec.name.clone(),
                resource_group: spec.resource_group.clone(),
                location: spec.location.clone(),
                size_gb: spec.size_gb,
                sku: spec.sku.clone(),
            })
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(created))
    }
}
