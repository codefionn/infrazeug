//! Ensure a persistent disk exists.

use crate::client::GcpClientSource;
use async_trait::async_trait;
use infrazeug_ext_gcp_api::compute::{Disk, DiskCreate};
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_DISK: &str = "gcp.ensure_disk";

pub type EnsureDisk = EnsureResource<DiskResource>;

pub fn ensure_disk(source: GcpClientSource) -> EnsureDisk {
    EnsureResource::new(DiskResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureDiskInput {
    pub name: String,
    pub zone: String,
    pub size_gb: u32,
    pub disk_type: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureDiskOutput {
    pub disk_id: String,
    pub name: String,
    pub zone: String,
    pub size_gb: u32,
}

#[derive(Clone)]
pub struct DiskResource {
    source: GcpClientSource,
}

impl DiskResource {
    pub fn new(source: GcpClientSource) -> Self {
        Self { source }
    }
}

fn to_output(disk: Disk) -> EnsureDiskOutput {
    EnsureDiskOutput {
        disk_id: disk.disk_id,
        name: disk.name,
        zone: disk.zone,
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
            .compute_disks(&spec.zone)
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
            .compute_disk_create(&DiskCreate {
                name: spec.name.clone(),
                zone: spec.zone.clone(),
                size_gb: spec.size_gb,
                disk_type: spec.disk_type.clone(),
            })
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(created))
    }
}
