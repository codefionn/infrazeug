//! AWS tier-1 native nodes for infrazeug playbooks.
//!
//! Thin bridge over [`infrazeug_ext_aws_api`]: registers
//! [`Resource`](infrazeug_resource::Resource) implementations as tier-1
//! node methods and exposes [`AwsInfraBuilder`] helpers.

mod builder;
mod client;
mod methods;
mod registry;

pub use builder::{AwsInfraBuilder, AwsInfraExt};
pub use client::{client_from_env, AwsClientSource};
pub use methods::{
    ensure_bucket, ensure_iam_access_key, ensure_instance, ensure_volume, EnsureBucket,
    EnsureBucketInput, EnsureBucketOutput, EnsureIamAccessKey, EnsureIamAccessKeyInput,
    EnsureIamAccessKeyOutput, EnsureInstance, EnsureInstanceInput, EnsureInstanceOutput,
    EnsureVolume, EnsureVolumeInput, EnsureVolumeOutput, ENSURE_BUCKET, ENSURE_IAM_ACCESS_KEY,
    ENSURE_INSTANCE, ENSURE_VOLUME,
};
pub use registry::method_registry;

pub use infrazeug_ext_aws_api::{AwsClient, AwsConfig, AwsCredentials};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
