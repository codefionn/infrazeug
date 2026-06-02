//! OpenStack tier-1 native nodes for infrazeug playbooks.
//!
//! Thin bridge over [`infrazeug_ext_openstack`]: registers [`NodeMethod`] implementations
//! and exposes [`OpenstackInfraBuilder`] helpers for idempotent S3 backup provisioning
//! via Keystone EC2 credentials and SigV4 bucket operations.

mod builder;
mod client;
mod methods;
mod registry;
mod vault;

pub use builder::{OpenstackBackupStack, OpenstackInfraBuilder, OpenstackInfraExt};
pub use client::OpenstackClientSource;
pub use methods::{
    ensure_bucket, ensure_s3_credentials, EnsureBucket, EnsureBucketInput, EnsureBucketOutput,
    EnsureS3Credentials, EnsureS3CredentialsInput, EnsureS3CredentialsOutput, ENSURE_BUCKET,
    ENSURE_S3_CREDENTIALS,
};
pub use registry::method_registry;
pub use vault::MutableVaultTarget;

pub use infrazeug_ext_openstack::{OpenstackClient, OpenstackConfig};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
