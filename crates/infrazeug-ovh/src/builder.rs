//! Fluent infra builder extension for OVH native nodes.

use crate::client::OvhClientSource;
use crate::methods::{
    ensure_instance, ensure_s3_user, ensure_s3_user_policy, ensure_storage_container,
    EnsureInstance, EnsureInstanceInput, EnsureS3User, EnsureS3UserInput, EnsureS3UserPolicy,
    EnsureS3UserPolicyInput, EnsureStorageContainer, EnsureStorageContainerInput,
};
use crate::vault::{vault_field_from_native_capture, MutableVaultTarget};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::RunPolicy;
use infrazeug_ext_ovh_api::OvhClient;
use uuid::Uuid;

/// Composite recipe: object-storage container + S3 user + mutable vault writes.
#[derive(Clone, Debug)]
pub struct BackupStack {
    pub project_id: String,
    pub container_name: String,
    pub region: String,
    pub user_description: String,
    pub bucket_node_id: NodeId,
    pub user_node_id: NodeId,
    pub user_policy_node_id: NodeId,
    /// When set, adds standard `VaultWrite` shell nodes after the S3 user native node.
    pub vault: Option<MutableVaultTarget>,
    pub vault_access_key_node_id: NodeId,
    pub vault_secret_key_node_id: NodeId,
    /// Prefix for the generated node names; must differ between stacks registered on the
    /// same builder (node names are unique). Defaults to `ovh-backup`.
    pub node_name_prefix: String,
}

impl BackupStack {
    pub fn new(
        project_id: impl Into<String>,
        container_name: impl Into<String>,
        region: impl Into<String>,
        user_description: impl Into<String>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            container_name: container_name.into(),
            region: region.into(),
            user_description: user_description.into(),
            bucket_node_id: NodeId(Uuid::new_v4()),
            user_node_id: NodeId(Uuid::new_v4()),
            user_policy_node_id: NodeId(Uuid::new_v4()),
            vault: None,
            vault_access_key_node_id: NodeId(Uuid::new_v4()),
            vault_secret_key_node_id: NodeId(Uuid::new_v4()),
            node_name_prefix: "ovh-backup".into(),
        }
    }

    pub fn with_node_name_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.node_name_prefix = prefix.into();
        self
    }

    pub fn with_node_ids(mut self, bucket: NodeId, user: NodeId) -> Self {
        self.bucket_node_id = bucket;
        self.user_node_id = user;
        self
    }

    /// Store S3 credentials in `files/mutable/{file}` using DataKey `data_key_id`.
    pub fn with_mutable_vault(
        mut self,
        data_key_id: impl Into<String>,
        file: impl Into<String>,
    ) -> Self {
        self.vault = Some(MutableVaultTarget::new(data_key_id, file));
        self
    }
}

/// Extension trait: attach OVH methods to an [`InfraBuilder`].
pub trait OvhInfraExt {
    /// Register OVH methods against a ready client (e.g. from `client_from_env`).
    fn ovh(self, client: OvhClient, machine_id: MachineId) -> OvhInfraBuilder;

    /// Register OVH methods that read their credentials from the controller vault
    /// at apply time (no `OVH_*` environment variables needed).
    ///
    /// `file` is the vault file (under `files/`) holding `application_key`,
    /// `application_secret`, `consumer_key`, and an optional `endpoint`; its DataKey
    /// must be among the run's unlocked keys.
    fn ovh_vault(self, file: impl Into<String>, machine_id: MachineId) -> OvhInfraBuilder;

    /// Like [`ovh_vault`](Self::ovh_vault) but OAuth2 service-account credentials.
    ///
    /// Reads `client_id_field`/`client_secret_field` from `file` in the controller vault at
    /// apply time and authenticates via OAuth2 (endpoint EU). Field names are configurable so
    /// existing vault fields can be reused without re-sealing; the file's DataKey must be
    /// among the run's unlocked keys.
    fn ovh_vault_oauth2(
        self,
        file: impl Into<String>,
        client_id_field: impl Into<String>,
        client_secret_field: impl Into<String>,
        machine_id: MachineId,
    ) -> OvhInfraBuilder;
}

impl OvhInfraExt for InfraBuilder {
    fn ovh(self, client: OvhClient, machine_id: MachineId) -> OvhInfraBuilder {
        OvhInfraBuilder::new(self, OvhClientSource::ready(client), machine_id)
    }

    fn ovh_vault(self, file: impl Into<String>, machine_id: MachineId) -> OvhInfraBuilder {
        OvhInfraBuilder::new(self, OvhClientSource::vault(file), machine_id)
    }

    fn ovh_vault_oauth2(
        self,
        file: impl Into<String>,
        client_id_field: impl Into<String>,
        client_secret_field: impl Into<String>,
        machine_id: MachineId,
    ) -> OvhInfraBuilder {
        OvhInfraBuilder::new(
            self,
            OvhClientSource::vault_oauth2(file, client_id_field, client_secret_field),
            machine_id,
        )
    }
}

/// Staged builder with OVH methods pre-registered.
pub struct OvhInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl OvhInfraBuilder {
    pub fn new(builder: InfraBuilder, source: OvhClientSource, machine_id: MachineId) -> Self {
        let builder = builder
            .method(ensure_storage_container(source.clone()))
            .method(ensure_s3_user(source.clone()))
            .method(ensure_s3_user_policy(source.clone()))
            .method(ensure_instance(source));
        Self {
            builder,
            machine_id,
        }
    }

    /// Ensure a Public Cloud compute instance exists (create or skip).
    ///
    /// Captured outputs (instance id, IPs) are available for downstream nodes; the
    /// instance is not registered as an infrazeug machine.
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

    /// Ensure an object-storage container exists (create or skip).
    pub fn ensure_storage_container(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureStorageContainerInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureStorageContainer>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Ensure a project user with at least one S3 credential exists.
    pub fn ensure_s3_user(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureS3UserInput,
    ) -> anyhow::Result<Self> {
        self.ensure_s3_user_with_policy(node_id, name, input, RunPolicy::Always)
    }

    /// Like [`ensure_s3_user`](Self::ensure_s3_user) with an explicit run policy.
    pub fn ensure_s3_user_with_policy(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureS3UserInput,
        run_policy: RunPolicy,
    ) -> anyhow::Result<Self> {
        let staged =
            self.builder
                .native_typed::<EnsureS3User>(node_id, name, self.machine_id, input)?;
        let staged = match run_policy {
            RunPolicy::Always => staged.always(),
            RunPolicy::OnUpstreamChange => staged.on_upstream_change(),
            RunPolicy::Lazy => staged.lazy(),
        };
        let builder = staged.build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Ensure an S3 user after `deps` complete (ordering only; runs every apply).
    pub fn ensure_s3_user_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureS3UserInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureS3User>(node_id, name, self.machine_id, input)?
            .deps(deps)
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Ensure an Object Storage S3 policy for a project user after `deps` complete.
    pub fn ensure_s3_user_policy_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureS3UserPolicyInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureS3UserPolicy>(node_id, name, self.machine_id, input)?
            .deps(deps)
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Bucket then S3 user (user depends on bucket node, gated on upstream change).
    pub fn ensure_backup_stack(self, stack: BackupStack) -> anyhow::Result<Self> {
        let bucket_input = EnsureStorageContainerInput {
            project_id: stack.project_id.clone(),
            container_name: stack.container_name.clone(),
            region: stack.region.clone(),
        };
        let user_input = EnsureS3UserInput {
            project_id: stack.project_id,
            description: stack.user_description,
            role_names: vec![],
        };
        let policy_input = EnsureS3UserPolicyInput {
            project_id: bucket_input.project_id.clone(),
            user_description: user_input.description.clone(),
            policy: backup_bucket_policy(&bucket_input.container_name),
        };

        let prefix = &stack.node_name_prefix;
        let after_bucket = self.ensure_storage_container(
            stack.bucket_node_id,
            &format!("{prefix}-bucket"),
            bucket_input,
        )?;

        let after_user = after_bucket.ensure_s3_user_after(
            stack.user_node_id,
            &format!("{prefix}-s3-user"),
            user_input,
            [stack.bucket_node_id],
        )?;

        let after_policy = after_user.ensure_s3_user_policy_after(
            stack.user_policy_node_id,
            &format!("{prefix}-s3-user-policy"),
            policy_input,
            [stack.user_node_id],
        )?;

        let Some(vault) = stack.vault else {
            return Ok(after_policy);
        };

        let deps = [stack.user_node_id, stack.user_policy_node_id];
        let after_access = vault_field_from_native_capture(
            after_policy.builder,
            stack.vault_access_key_node_id,
            &format!("{prefix}-vault-s3-access-key"),
            after_policy.machine_id,
            &vault,
            "credentials.access_key",
            "/access_key_id",
            false,
            stack.user_node_id,
            deps,
        )?;
        // The S3 secret is returned by OVH only when the credential is first
        // issued; on a re-run against an existing credential the capture omits
        // it. Mark this write optional so it skips rather than failing.
        let builder = vault_field_from_native_capture(
            after_access,
            stack.vault_secret_key_node_id,
            &format!("{prefix}-vault-s3-secret-key"),
            after_policy.machine_id,
            &vault,
            "credentials.secret_key",
            "/secret_access_key",
            true,
            stack.user_node_id,
            deps,
        )?;
        Ok(Self {
            builder,
            machine_id: after_policy.machine_id,
        })
    }

    /// Return the underlying [`InfraBuilder`] for further chaining.
    pub fn into_builder(self) -> InfraBuilder {
        self.builder
    }

    /// Finish as a [`PlaybookBundle`].
    pub fn finish(self) -> PlaybookBundle {
        self.builder.build()
    }
}

fn backup_bucket_policy(bucket: &str) -> String {
    serde_json::json!({
        "Statement": [{
            "Sid": "BackupBucketReadWrite",
            "Effect": "Allow",
            "Action": [
                "s3:GetObject",
                "s3:PutObject",
                "s3:DeleteObject",
                "s3:ListBucket",
                "s3:ListMultipartUploadParts",
                "s3:ListBucketMultipartUploads",
                "s3:AbortMultipartUpload",
                "s3:GetBucketLocation"
            ],
            "Resource": [
                format!("arn:aws:s3:::{bucket}"),
                format!("arn:aws:s3:::{bucket}/*")
            ]
        }]
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_api::builder;
    use infrazeug_ext_ovh_api::{Credentials, OvhEndpoint};

    fn dummy_client() -> OvhClient {
        OvhClient::from_credentials(OvhEndpoint::OvhEu, Credentials::new("ak", "as", "ck"))
    }

    #[test]
    fn backup_stack_plans() {
        let local = MachineId(Uuid::new_v4());
        let bundle = InfraBuilder::new()
            .machine(builder::controller(local))
            .unwrap()
            .ovh(dummy_client(), local)
            .ensure_backup_stack(
                BackupStack::new("proj", "backups", "GRA", "backup-user")
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
        assert_eq!(real_nodes, 5);
        bundle.plan().expect("lint + plan");
    }

    #[test]
    fn ensure_instance_plans() {
        let local = MachineId(Uuid::new_v4());
        let node = NodeId(Uuid::new_v4());
        let bundle = InfraBuilder::new()
            .machine(builder::controller(local))
            .unwrap()
            .ovh(dummy_client(), local)
            .ensure_instance(
                node,
                "web-1",
                EnsureInstanceInput {
                    project_id: "proj".into(),
                    name: "web-1".into(),
                    region: "GRA11".into(),
                    flavor_id: "flavor-id".into(),
                    image_id: Some("image-id".into()),
                    ssh_key_id: None,
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
