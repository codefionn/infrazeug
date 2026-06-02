//! Ensure a STACKIT IaaS block volume exists (captured outputs only).

use crate::client::StackitClientSource;
use async_trait::async_trait;
use infrazeug_ext_stackit_api::types::ResourceSource;
use infrazeug_ext_stackit_api::volumes::{Volume, VolumeCreate};
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_VOLUME: &str = "stackit.ensure_volume";

pub type EnsureVolume = EnsureResource<VolumeResource>;

pub fn ensure_volume(source: StackitClientSource) -> EnsureVolume {
    EnsureResource::new(VolumeResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureVolumeInput {
    pub project_id: String,
    pub name: String,
    /// Size in GiB.
    pub size: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(default = "default_source_type")]
    pub source_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
}

fn default_source_type() -> String {
    "image".into()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureVolumeOutput {
    pub volume_id: String,
    pub name: String,
    #[serde(default)]
    pub size: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Clone)]
pub struct VolumeResource {
    source: StackitClientSource,
}

impl VolumeResource {
    pub fn new(source: StackitClientSource) -> Self {
        Self { source }
    }
}

fn to_output(volume: Volume) -> Option<EnsureVolumeOutput> {
    let id = volume.id?;
    Some(EnsureVolumeOutput {
        volume_id: id,
        name: volume.name.unwrap_or_default(),
        size: volume.size.unwrap_or_default(),
        status: volume.status,
    })
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
        let volumes = if let Some(region) = &spec.region {
            client
                .volumes_v2(&spec.project_id, region)
                .await
                .map_err(ResourceError::provider)?
        } else {
            client
                .volumes(&spec.project_id)
                .await
                .map_err(ResourceError::provider)?
        };
        Ok(volumes
            .items
            .into_iter()
            .find(|v| v.name.as_deref() == Some(spec.name.as_str()))
            .and_then(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let source = spec.source_id.as_ref().map(|id| ResourceSource {
            id: id.clone(),
            source_type: spec.source_type.clone(),
        });
        let body = VolumeCreate {
            name: spec.name.clone(),
            size: spec.size,
            availability_zone: spec.availability_zone.clone(),
            source,
            performance_class: spec.performance_class.clone(),
        };
        let created = if let Some(region) = &spec.region {
            client
                .create_volume_v2(&spec.project_id, region, &body)
                .await
                .map_err(ResourceError::provider)?
        } else {
            client
                .create_volume(&spec.project_id, &body)
                .await
                .map_err(ResourceError::provider)?
        };
        to_output(created).ok_or_else(|| ResourceError::provider("created volume has no id"))
    }
}
