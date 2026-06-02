//! OVHcloud tier-1 native nodes for infrazeug playbooks.
//!
//! Thin bridge over [`infrazeug_ext_ovh_api`]: registers [`NodeMethod`] implementations
//! and exposes [`OvhInfraBuilder`] helpers so playbooks do not hand-roll method
//! registration or idempotent ensure logic.
//!
//! [`BackupStack`] provisions OVH Public Cloud **S3 Object Storage**, not legacy
//! Swift/Object Storage containers. It uses the region-scoped storage API, ensures a
//! Public Cloud user with `s3Credentials`, and applies an S3 user policy granting
//! read/write access to the bucket. The policy step matters: an S3 access key can
//! exist while still receiving `403 Forbidden` from S3 until the user policy grants
//! bucket access.
//!
//! # Example
//!
//! ```ignore
//! use infrazeug_api::builder::{self, InfraBuilder};
//! use infrazeug_ovh::{client_from_env, BackupStack, OvhInfraExt};
//! use infrazeug_core::id::MachineId;
//! use uuid::Uuid;
//!
//! let local = MachineId(Uuid::new_v4());
//! let bundle = InfraBuilder::new()
//!     .machine(builder::controller(local))?
//!     .ovh(client_from_env()?, local)
//!     .ensure_backup_stack(BackupStack::new("project-id", "backups", "GRA", "backup-user"))?
//!     .finish();
//! ```

mod builder;
mod client;
mod methods;
mod registry;
mod vault;

pub use builder::{BackupStack, OvhInfraBuilder, OvhInfraExt};
pub use client::{client_from_env, OvhClientSource};
pub use methods::{
    ensure_instance, ensure_s3_user, ensure_s3_user_policy, ensure_storage_container,
    EnsureInstance, EnsureInstanceInput, EnsureInstanceOutput, EnsureS3User, EnsureS3UserInput,
    EnsureS3UserOutput, EnsureS3UserPolicy, EnsureS3UserPolicyInput, EnsureS3UserPolicyOutput,
    EnsureStorageContainer, EnsureStorageContainerInput, EnsureStorageContainerOutput,
    ENSURE_INSTANCE, ENSURE_S3_USER, ENSURE_S3_USER_POLICY, ENSURE_STORAGE_CONTAINER,
};
pub use registry::method_registry;
pub use vault::MutableVaultTarget;

pub use infrazeug_ext_ovh_api::{OvhClient, OvhEndpoint};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
