//! Tier-1 resource methods for OpenStack.

mod bucket;
mod s3_credentials;

pub use bucket::{
    ensure_bucket, EnsureBucket, EnsureBucketInput, EnsureBucketOutput, ENSURE_BUCKET,
};
pub use s3_credentials::{
    ensure_s3_credentials, EnsureS3Credentials, EnsureS3CredentialsInput,
    EnsureS3CredentialsOutput, ENSURE_S3_CREDENTIALS,
};
