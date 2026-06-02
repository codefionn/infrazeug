//! Ensure an EC2 instance exists (captured outputs only).

use crate::client::AwsClientSource;
use async_trait::async_trait;
use infrazeug_ext_aws_api::ec2::{Instance, InstanceCreate};
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_INSTANCE: &str = "aws.ensure_instance";

pub type EnsureInstance = EnsureResource<InstanceResource>;

pub fn ensure_instance(source: AwsClientSource) -> EnsureInstance {
    EnsureResource::new(InstanceResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureInstanceInput {
    pub name: String,
    pub image_id: String,
    pub instance_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subnet_id: Option<String>,
    #[serde(default)]
    pub security_group_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureInstanceOutput {
    pub instance_id: String,
    pub name: String,
    #[serde(default)]
    pub private_ip: Option<String>,
    #[serde(default)]
    pub public_ip: Option<String>,
}

#[derive(Clone)]
pub struct InstanceResource {
    source: AwsClientSource,
}

impl InstanceResource {
    pub fn new(source: AwsClientSource) -> Self {
        Self { source }
    }
}

fn to_output(instance: Instance) -> EnsureInstanceOutput {
    EnsureInstanceOutput {
        instance_id: instance.instance_id,
        name: instance.name,
        private_ip: instance.private_ip,
        public_ip: instance.public_ip,
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
            .ec2_instances(&spec.name)
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
            .ec2_instance_create(&InstanceCreate {
                image_id: spec.image_id.clone(),
                instance_type: spec.instance_type.clone(),
                name: spec.name.clone(),
                key_name: spec.key_name.clone(),
                subnet_id: spec.subnet_id.clone(),
                security_group_ids: if spec.security_group_ids.is_empty() {
                    None
                } else {
                    Some(spec.security_group_ids.clone())
                },
            })
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(created))
    }
}
