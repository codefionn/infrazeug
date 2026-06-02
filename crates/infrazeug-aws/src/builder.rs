//! Fluent infra builder extension for AWS native nodes.

use crate::client::AwsClientSource;
use crate::methods::{
    ensure_bucket, ensure_iam_access_key, ensure_instance, ensure_volume, EnsureBucket,
    EnsureBucketInput, EnsureIamAccessKey, EnsureIamAccessKeyInput, EnsureInstance,
    EnsureInstanceInput, EnsureVolume, EnsureVolumeInput,
};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_ext_aws_api::AwsClient;

/// Extension trait: attach AWS methods to an [`InfraBuilder`].
pub trait AwsInfraExt {
    fn aws(self, client: AwsClient, machine_id: MachineId) -> AwsInfraBuilder;
    fn aws_vault(self, file: impl Into<String>, machine_id: MachineId) -> AwsInfraBuilder;
}

impl AwsInfraExt for InfraBuilder {
    fn aws(self, client: AwsClient, machine_id: MachineId) -> AwsInfraBuilder {
        AwsInfraBuilder::new(self, AwsClientSource::ready(client), machine_id)
    }

    fn aws_vault(self, file: impl Into<String>, machine_id: MachineId) -> AwsInfraBuilder {
        AwsInfraBuilder::new(self, AwsClientSource::vault(file), machine_id)
    }
}

/// Staged builder with AWS methods pre-registered.
pub struct AwsInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl AwsInfraBuilder {
    pub fn new(builder: InfraBuilder, source: AwsClientSource, machine_id: MachineId) -> Self {
        let builder = builder
            .method(ensure_instance(source.clone()))
            .method(ensure_volume(source.clone()))
            .method(ensure_bucket(source.clone()))
            .method(ensure_iam_access_key(source));
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

    pub fn ensure_volume(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureVolumeInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureVolume>(node_id, name, self.machine_id, input)?
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

    pub fn ensure_iam_access_key(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureIamAccessKeyInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureIamAccessKey>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    pub fn into_builder(self) -> InfraBuilder {
        self.builder
    }

    pub fn finish(self) -> PlaybookBundle {
        self.builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_api::builder;
    use infrazeug_ext_aws_api::{AwsConfig, AwsCredentials};
    use uuid::Uuid;

    fn dummy_client() -> AwsClient {
        AwsClient::new(AwsConfig::new(AwsCredentials::new("ak", "sk"), "us-east-1"))
    }

    #[test]
    fn aws_resources_plan() {
        let local = MachineId(Uuid::new_v4());
        let bundle = InfraBuilder::new()
            .machine(builder::controller(local))
            .unwrap()
            .aws(dummy_client(), local)
            .ensure_instance(
                NodeId(Uuid::new_v4()),
                "web-1",
                EnsureInstanceInput {
                    name: "web-1".into(),
                    image_id: "ami-123".into(),
                    instance_type: "t3.micro".into(),
                    key_name: None,
                    subnet_id: None,
                    security_group_ids: vec![],
                },
            )
            .unwrap()
            .finish();

        // `build()` injects a per-machine connectivity head plus the global
        // begin/finish bookends; count the real (user-authored) nodes.
        let real_nodes = bundle
            .infra
            .nodes
            .iter()
            .filter(|n| !(n.body.is_group_bookend() || n.body.is_connect()))
            .count();
        assert_eq!(real_nodes, 1);
        bundle.plan().expect("lint + plan");
    }
}
