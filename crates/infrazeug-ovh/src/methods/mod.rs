//! Tier-1 resource methods for OVH Public Cloud.
//!
//! Each resource implements [`infrazeug_resource::Resource`] and is exposed as a
//! [`NodeMethod`](infrazeug_native::NodeMethod) via
//! [`EnsureResource`](infrazeug_resource::EnsureResource).

mod instance;
mod s3_policy;
mod s3_user;
mod storage;

pub use instance::{
    ensure_instance, EnsureInstance, EnsureInstanceInput, EnsureInstanceOutput, ENSURE_INSTANCE,
};
pub use s3_policy::{
    ensure_s3_user_policy, EnsureS3UserPolicy, EnsureS3UserPolicyInput, EnsureS3UserPolicyOutput,
    ENSURE_S3_USER_POLICY,
};
pub use s3_user::{
    ensure_s3_user, EnsureS3User, EnsureS3UserInput, EnsureS3UserOutput, ENSURE_S3_USER,
};
pub use storage::{
    ensure_storage_container, EnsureStorageContainer, EnsureStorageContainerInput,
    EnsureStorageContainerOutput, ENSURE_STORAGE_CONTAINER,
};
