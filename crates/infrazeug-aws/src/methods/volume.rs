//! Ensure an EBS volume exists (captured outputs only).

use crate::client::AwsClientSource;
use async_trait::async_trait;
use infrazeug_ext_aws_api::ec2::{Volume, VolumeCreate};
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_VOLUME: &str = "aws.ensure_volume";

pub type EnsureVolume = EnsureResource<VolumeResource>;

pub fn ensure_volume(source: AwsClientSource) -> EnsureVolume {
    EnsureResource::new(VolumeResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureVolumeInput {
    pub name: String,
    pub availability_zone: String,
    pub size: u32,
    pub volume_type: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureVolumeOutput {
    pub volume_id: String,
    pub name: String,
    pub size: u32,
}

#[derive(Clone)]
pub struct VolumeResource {
    source: AwsClientSource,
}

impl VolumeResource {
    pub fn new(source: AwsClientSource) -> Self {
        Self { source }
    }
}

fn to_output(volume: Volume) -> EnsureVolumeOutput {
    EnsureVolumeOutput {
        volume_id: volume.volume_id,
        name: volume.name,
        size: volume.size,
    }
}

#[async_trait]
impl Resource for VolumeResource {
    type Spec = EnsureVolumeInput;
    type State = EnsureVolumeOutput;

    fn kind(&self) -> &'static str {
        ENSURE_VOLUME
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let volumes = client
            .ec2_volumes(&spec.name)
            .await
            .map_err(ResourceError::provider)?;
        Ok(volumes
            .into_iter()
            .find(|v| v.name == spec.name)
            .map(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let created = client
            .ec2_volume_create(&VolumeCreate {
                name: spec.name.clone(),
                availability_zone: spec.availability_zone.clone(),
                size: spec.size,
                volume_type: spec.volume_type.clone(),
            })
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(created))
    }
}
