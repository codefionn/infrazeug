//! Ensure a Compute Engine instance exists.

use crate::client::GcpClientSource;
use async_trait::async_trait;
use infrazeug_ext_gcp_api::compute::{Instance, InstanceCreate};
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_INSTANCE: &str = "gcp.ensure_instance";

pub type EnsureInstance = EnsureResource<InstanceResource>;

pub fn ensure_instance(source: GcpClientSource) -> EnsureInstance {
    EnsureResource::new(InstanceResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureInstanceInput {
    pub name: String,
    pub zone: String,
    pub machine_type: String,
    pub source_image: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_size_gb: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureInstanceOutput {
    pub instance_id: String,
    pub name: String,
    pub zone: String,
    #[serde(default)]
    pub internal_ip: Option<String>,
    #[serde(default)]
    pub external_ip: Option<String>,
}

#[derive(Clone)]
pub struct InstanceResource {
    source: GcpClientSource,
}

impl InstanceResource {
    pub fn new(source: GcpClientSource) -> Self {
        Self { source }
    }
}

fn to_output(instance: Instance) -> EnsureInstanceOutput {
    EnsureInstanceOutput {
        instance_id: instance.instance_id,
        name: instance.name,
        zone: instance.zone,
        internal_ip: instance.internal_ip,
        external_ip: instance.external_ip,
    }
}

#[async_trait]
impl Resource for InstanceResource {
    type Spec = EnsureInstanceInput;
    type State = EnsureInstanceOutput;

    fn kind(&self) -> &'static str {
        ENSURE_INSTANCE
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let instances = client
            .compute_instances(&spec.zone)
            .await
            .map_err(ResourceError::provider)?;
        Ok(instances
            .into_iter()
            .find(|i| i.name == spec.name)
            .map(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let created = client
            .compute_instance_create(&InstanceCreate {
                name: spec.name.clone(),
                zone: spec.zone.clone(),
                machine_type: spec.machine_type.clone(),
                source_image: spec.source_image.clone(),
                disk_size_gb: spec.disk_size_gb,
            })
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(created))
    }
}
