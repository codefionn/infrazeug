//! Tier-1 resource methods for AWS.

mod bucket;
mod iam_access_key;
mod instance;
mod volume;

pub use bucket::{
    ensure_bucket, EnsureBucket, EnsureBucketInput, EnsureBucketOutput, ENSURE_BUCKET,
};
pub use iam_access_key::{
    ensure_iam_access_key, EnsureIamAccessKey, EnsureIamAccessKeyInput, EnsureIamAccessKeyOutput,
    ENSURE_IAM_ACCESS_KEY,
};
pub use instance::{
    ensure_instance, EnsureInstance, EnsureInstanceInput, EnsureInstanceOutput, ENSURE_INSTANCE,
};
pub use volume::{
    ensure_volume, EnsureVolume, EnsureVolumeInput, EnsureVolumeOutput, ENSURE_VOLUME,
};
