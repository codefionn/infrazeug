//! Fluent infra builder extension for OpenStack native nodes.

use crate::client::OpenstackClientSource;
use crate::methods::{
    ensure_bucket, ensure_s3_credentials, EnsureBucket, EnsureBucketInput, EnsureS3Credentials,
    EnsureS3CredentialsInput,
};
use crate::vault::{vault_field_from_native_capture, MutableVaultTarget};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::RunPolicy;
use infrazeug_ext_openstack::{OpenstackClient, OpenstackConfig};
use uuid::Uuid;

/// Composite recipe: EC2/S3 credentials + mutable vault writes + S3 bucket.
#[derive(Clone, Debug)]
pub struct OpenstackBackupStack {
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
    /// Mutable vault file path for credential reads (e.g. `mutable/cloud/backups.vault`).
    pub creds_file: String,
    pub credentials_node_id: NodeId,
    pub bucket_node_id: NodeId,
    pub vault_access_key_node_id: NodeId,
    pub vault_secret_key_node_id: NodeId,
    pub vault: Option<MutableVaultTarget>,
}

impl OpenstackBackupStack {
    pub fn new(
        bucket: impl Into<String>,
        region: impl Into<String>,
        endpoint: impl Into<String>,
        creds_file: impl Into<String>,
    ) -> Self {
        Self {
            bucket: bucket.into(),
            region: region.into(),
            endpoint: endpoint.into(),
            creds_file: creds_file.into(),
            credentials_node_id: NodeId(Uuid::new_v4()),
            bucket_node_id: NodeId(Uuid::new_v4()),
            vault_access_key_node_id: NodeId(Uuid::new_v4()),
            vault_secret_key_node_id: NodeId(Uuid::new_v4()),
            vault: None,
        }
    }

    pub fn with_node_ids(
        mut self,
        credentials: NodeId,
        bucket: NodeId,
        vault_access: NodeId,
        vault_secret: NodeId,
    ) -> Self {
        self.credentials_node_id = credentials;
        self.bucket_node_id = bucket;
        self.vault_access_key_node_id = vault_access;
        self.vault_secret_key_node_id = vault_secret;
        self
    }

    pub fn with_mutable_vault(
        mut self,
        data_key_id: impl Into<String>,
        file: impl Into<String>,
    ) -> Self {
        self.vault = Some(MutableVaultTarget::new(data_key_id, file));
        self
    }
}

/// Extension trait: attach OpenStack methods to an [`InfraBuilder`].
pub trait OpenstackInfraExt {
    fn openstack(self, client: OpenstackClient, machine_id: MachineId) -> OpenstackInfraBuilder;

    fn openstack_vault(
        self,
        file: impl Into<String>,
        username_field: impl Into<String>,
        password_field: impl Into<String>,
        config: OpenstackConfig,
        machine_id: MachineId,
    ) -> OpenstackInfraBuilder;
}

impl OpenstackInfraExt for InfraBuilder {
    fn openstack(self, client: OpenstackClient, machine_id: MachineId) -> OpenstackInfraBuilder {
        OpenstackInfraBuilder::new(self, OpenstackClientSource::ready(client), machine_id)
    }

    fn openstack_vault(
        self,
        file: impl Into<String>,
        username_field: impl Into<String>,
        password_field: impl Into<String>,
        config: OpenstackConfig,
        machine_id: MachineId,
    ) -> OpenstackInfraBuilder {
        OpenstackInfraBuilder::new(
            self,
            OpenstackClientSource::vault(file, username_field, password_field, config),
            machine_id,
        )
    }
}

pub struct OpenstackInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl OpenstackInfraBuilder {
    pub fn new(
        builder: InfraBuilder,
        source: OpenstackClientSource,
        machine_id: MachineId,
    ) -> Self {
        let builder = builder
            .method(ensure_s3_credentials(source.clone()))
            .method(ensure_bucket(source));
        Self {
            builder,
            machine_id,
        }
    }

    pub fn ensure_s3_credentials_with_policy(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureS3CredentialsInput,
        run_policy: RunPolicy,
    ) -> anyhow::Result<Self> {
        let staged = self.builder.native_typed::<EnsureS3Credentials>(
            node_id,
            name,
            self.machine_id,
            input,
        )?;
        let staged = match run_policy {
            RunPolicy::Always => staged.always(),
            RunPolicy::OnUpstreamChange => staged.on_upstream_change(),
            RunPolicy::Lazy => staged.lazy(),
        };
        Ok(Self {
            builder: staged.build()?,
            machine_id: self.machine_id,
        })
    }

    pub fn ensure_bucket_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureBucketInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureBucket>(node_id, name, self.machine_id, input)?
            .deps(deps)
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// EC2 credentials → vault writes → S3 bucket (reads creds from mutable vault).
    pub fn ensure_backup_stack(self, stack: OpenstackBackupStack) -> anyhow::Result<Self> {
        let after_creds = self.ensure_s3_credentials_with_policy(
            stack.credentials_node_id,
            "openstack-s3-credentials",
            EnsureS3CredentialsInput {},
            RunPolicy::Always,
        )?;

        let Some(vault) = stack.vault else {
            let bucket_input = EnsureBucketInput {
                bucket: stack.bucket,
                region: stack.region,
                endpoint: stack.endpoint,
                creds_file: stack.creds_file,
            };
            return after_creds.ensure_bucket_after(
                stack.bucket_node_id,
                "openstack-backup-bucket",
                bucket_input,
                [stack.credentials_node_id],
            );
        };

        let deps = [stack.credentials_node_id];
        let after_access = vault_field_from_native_capture(
            after_creds.builder,
            stack.vault_access_key_node_id,
            "vault-s3-access-key",
            after_creds.machine_id,
            &vault,
            "credentials.access_key",
            "/access_key_id",
            false,
            stack.credentials_node_id,
            deps,
        )?;
        let after_secret = vault_field_from_native_capture(
            after_access,
            stack.vault_secret_key_node_id,
            "vault-s3-secret-key",
            after_creds.machine_id,
            &vault,
            "credentials.secret_key",
            "/secret_access_key",
            true,
            stack.credentials_node_id,
            deps,
        )?;

        let bucket_input = EnsureBucketInput {
            bucket: stack.bucket,
            region: stack.region,
            endpoint: stack.endpoint,
            creds_file: stack.creds_file,
        };
        let builder = after_secret
            .native_typed::<EnsureBucket>(
                stack.bucket_node_id,
                "openstack-backup-bucket",
                after_creds.machine_id,
                bucket_input,
            )?
            .deps([
                stack.vault_access_key_node_id,
                stack.vault_secret_key_node_id,
            ])
            .always()
            .build()?;

        Ok(Self {
            builder,
            machine_id: after_creds.machine_id,
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

    async fn dummy_client() -> OpenstackClient {
        let client = OpenstackClient::new(OpenstackConfig::ovh_public_cloud("proj", "DE"));
        // Tests only plan/lint — no live auth.
        client
    }

    #[tokio::test]
    async fn backup_stack_plans() {
        let local = MachineId(Uuid::new_v4());
        let bundle = InfraBuilder::new()
            .machine(builder::controller(local))
            .unwrap()
            .openstack(dummy_client().await, local)
            .ensure_backup_stack(
                OpenstackBackupStack::new(
                    "backups",
                    "de",
                    "https://s3.de.io.cloud.ovh.net",
                    "mutable/cloud/backups.vault",
                )
                .with_mutable_vault("prod-runtime", "cloud/backups.vault"),
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
        assert_eq!(real_nodes, 4);
        bundle.plan().expect("lint + plan");
    }
}
