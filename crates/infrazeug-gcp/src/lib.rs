//! GCP tier-1 native nodes for infrazeug playbooks.

mod builder;
mod client;
mod methods;
mod registry;

pub use builder::{GcpInfraBuilder, GcpInfraExt};
pub use client::{client_from_env, GcpClientSource};
pub use methods::{
    ensure_bucket, ensure_disk, ensure_instance, ensure_service_account_key, EnsureBucket,
    EnsureBucketInput, EnsureBucketOutput, EnsureDisk, EnsureDiskInput, EnsureDiskOutput,
    EnsureInstance, EnsureInstanceInput, EnsureInstanceOutput, EnsureServiceAccountKey,
    EnsureServiceAccountKeyInput, EnsureServiceAccountKeyOutput, ENSURE_BUCKET, ENSURE_DISK,
    ENSURE_INSTANCE, ENSURE_SERVICE_ACCOUNT_KEY,
};
pub use registry::method_registry;

pub use infrazeug_ext_gcp_api::{GcpAuth, GcpClient, GcpConfig, ServiceAccountKey};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
