//! Ensure a Public Cloud compute instance exists (captured outputs only).
//!
//! The created instance's id and IP addresses are captured for downstream
//! vault/file nodes; wiring a provisioned instance into the machine/transport
//! layer as an SSH target is a separate concern (not done here).

use crate::client::OvhClientSource;
use async_trait::async_trait;
use infrazeug_ext_ovh_api::public_cloud::{Instance, InstanceCreate, InstanceListQuery};
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_INSTANCE: &str = "ovh.ensure_instance";

/// Tier-1 method: ensure an OVH Public Cloud instance.
pub type EnsureInstance = EnsureResource<InstanceResource>;

/// Construct the registrable [`EnsureInstance`] method for a client source.
pub fn ensure_instance(source: OvhClientSource) -> EnsureInstance {
    EnsureResource::new(InstanceResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureInstanceInput {
    pub project_id: String,
    pub name: String,
    pub region: String,
    pub flavor_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_key_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureInstanceOutput {
    pub instance_id: String,
    pub name: String,
    pub region: String,
    /// IPv4 addresses known at observe/create time (may be empty while provisioning).
    #[serde(default)]
    pub ipv4: Vec<String>,
    /// IPv6 addresses known at observe/create time.
    #[serde(default)]
    pub ipv6: Vec<String>,
}

/// OVH Public Cloud compute instance as an acquirable resource.
#[derive(Clone)]
pub struct InstanceResource {
    source: OvhClientSource,
}

impl InstanceResource {
    pub fn new(source: OvhClientSource) -> Self {
        Self { source }
    }
}

fn to_output(instance: Instance) -> EnsureInstanceOutput {
    let mut ipv4 = Vec::new();
    let mut ipv6 = Vec::new();
    for addr in instance.ip_addresses {
        match addr.version {
            Some(6) => ipv6.push(addr.ip),
            _ => ipv4.push(addr.ip),
        }
    }
    EnsureInstanceOutput {
        instance_id: instance.id,
        name: instance.name,
        region: instance.region,
        ipv4,
        ipv6,
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
            .cloud_instances(
                &spec.project_id,
                InstanceListQuery {
                    region: Some(&spec.region),
                },
            )
            .await
            .map_err(ResourceError::provider)?;
        Ok(instances
            .into_iter()
            .find(|i| i.name == spec.name && i.region == spec.region)
            .map(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let created = client
            .cloud_instance_create(
                &spec.project_id,
                &InstanceCreate {
                    name: spec.name.clone(),
                    region: spec.region.clone(),
                    flavor_id: spec.flavor_id.clone(),
                    image_id: spec.image_id.clone(),
                    ssh_key_id: spec.ssh_key_id.clone(),
                    ..Default::default()
                },
            )
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(created))
    }

    // `diff` stays `InSync`: flavor/image are recreate-only and not safely
    // reconcilable in place; name+region form identity.
}
