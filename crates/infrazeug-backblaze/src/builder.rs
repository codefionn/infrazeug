//! Fluent infra builder extension for Backblaze native nodes.

use crate::client::BackblazeClientSource;
use crate::methods::{
    ensure_application_key, ensure_bucket, EnsureApplicationKey, EnsureApplicationKeyInput,
    EnsureBucket, EnsureBucketInput,
};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_ext_backblaze_api::BackblazeClient;

/// Extension trait: attach Backblaze methods to an [`InfraBuilder`].
pub trait BackblazeInfraExt {
    /// Register Backblaze methods bound to a ready [`BackblazeClient`].
    fn backblaze(self, client: BackblazeClient, machine_id: MachineId) -> BackblazeInfraBuilder;

    /// Register Backblaze methods with credentials read from the controller vault
    /// at apply time (`application_key_id`, `application_key`).
    fn backblaze_vault(
        self,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> BackblazeInfraBuilder;

    /// Register Backblaze methods bound to a pre-built [`BackblazeClientSource`].
    fn backblaze_source(
        self,
        source: BackblazeClientSource,
        machine_id: MachineId,
    ) -> BackblazeInfraBuilder;
}

impl BackblazeInfraExt for InfraBuilder {
    fn backblaze(self, client: BackblazeClient, machine_id: MachineId) -> BackblazeInfraBuilder {
        BackblazeInfraBuilder::new(self, BackblazeClientSource::ready(client), machine_id)
    }

    fn backblaze_vault(
        self,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> BackblazeInfraBuilder {
        BackblazeInfraBuilder::new(self, BackblazeClientSource::vault(file), machine_id)
    }

    fn backblaze_source(
        self,
        source: BackblazeClientSource,
        machine_id: MachineId,
    ) -> BackblazeInfraBuilder {
        BackblazeInfraBuilder::new(self, source, machine_id)
    }
}

/// Staged builder with Backblaze methods pre-registered.
pub struct BackblazeInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl BackblazeInfraBuilder {
    pub fn new(
        builder: InfraBuilder,
        source: BackblazeClientSource,
        machine_id: MachineId,
    ) -> Self {
        let builder = builder
            .method(ensure_bucket(source.clone()))
            .method(ensure_application_key(source));
        Self {
            builder,
            machine_id,
        }
    }

    /// Ensure a B2 bucket exists (create or reconcile bucket type).
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

    /// Ensure a B2 application key exists (create-only; secret returned once).
    pub fn ensure_application_key(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureApplicationKeyInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureApplicationKey>(node_id, name, self.machine_id, input)?
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
    use infrazeug_ext_backblaze_api::{BackblazeClient, BackblazeConfig, Credentials};
    use uuid::Uuid;

    fn dummy_client() -> BackblazeClient {
        BackblazeClient::new(BackblazeConfig::new(Credentials::new("id", "key")))
    }

    #[test]
    fn ensure_bucket_plans() {
        let local = MachineId(Uuid::new_v4());
        let node = NodeId(Uuid::new_v4());
        let bundle = InfraBuilder::new()
            .machine(builder::controller(local))
            .unwrap()
            .backblaze(dummy_client(), local)
            .ensure_bucket(
                node,
                "logs",
                EnsureBucketInput {
                    bucket_name: "my-logs-bucket".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .finish();

        bundle.plan().expect("lint + plan");
    }
}
