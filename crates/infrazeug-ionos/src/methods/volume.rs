//! Ensure an IONOS Cloud block volume exists (captured outputs only).

use crate::client::IonosClientSource;
use async_trait::async_trait;
use infrazeug_ext_ionos_cloud_api::volumes::{Volume, VolumeCreate, VolumeCreateProperties};
use infrazeug_ext_ionos_cloud_api::ListQuery;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_VOLUME: &str = "ionos.ensure_volume";

/// Tier-1 method: ensure an IONOS block volume in a data center.
pub type EnsureVolume = EnsureResource<VolumeResource>;

/// Construct the registrable [`EnsureVolume`] method for a client source.
pub fn ensure_volume(source: IonosClientSource) -> EnsureVolume {
    EnsureResource::new(VolumeResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureVolumeInput {
    pub datacenter_id: String,
    pub name: String,
    /// Disk type, e.g. `HDD` or `SSD`.
    pub disk_type: String,
    /// Size in GiB.
    pub size: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureVolumeOutput {
    pub volume_id: String,
    pub name: String,
    #[serde(default)]
    pub size: u32,
}

/// IONOS Cloud block volume as an acquirable resource.
#[derive(Clone)]
pub struct VolumeResource {
    source: IonosClientSource,
}

impl VolumeResource {
    pub fn new(source: IonosClientSource) -> Self {
        Self { source }
    }
}

fn to_output(volume: Volume) -> Option<EnsureVolumeOutput> {
    let id = volume.id?;
    let props = volume.properties.unwrap_or_default();
    Some(EnsureVolumeOutput {
        volume_id: id,
        name: props.name.unwrap_or_default(),
        size: props.size.unwrap_or_default(),
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
        let volumes = client
            .volumes(&spec.datacenter_id, &ListQuery::default())
            .await
            .map_err(ResourceError::provider)?;
        Ok(volumes
            .items
            .into_iter()
            .find(|v| {
                v.properties.as_ref().and_then(|p| p.name.as_deref()) == Some(spec.name.as_str())
            })
            .and_then(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let body = VolumeCreate {
            properties: VolumeCreateProperties {
                name: spec.name.clone(),
                kind: spec.disk_type.clone(),
                size: spec.size,
                availability_zone: spec.availability_zone.clone(),
                image: spec.image.clone(),
                ..Default::default()
            },
            ..Default::default()
        };
        let created = client
            .create_volume(&spec.datacenter_id, &body, &ListQuery::default())
            .await
            .map_err(ResourceError::provider)?;
        to_output(created).ok_or_else(|| ResourceError::provider("created volume has no id"))
    }

    // `diff` stays `InSync`: volume resize is a dedicated action; name +
    // datacenter form identity here.
}
