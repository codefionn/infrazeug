//! Ensure an S3 bucket exists.

use crate::client::AwsClientSource;
use async_trait::async_trait;
use infrazeug_ext_aws_api::s3::Bucket;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_BUCKET: &str = "aws.ensure_bucket";

pub type EnsureBucket = EnsureResource<BucketResource>;

pub fn ensure_bucket(source: AwsClientSource) -> EnsureBucket {
    EnsureResource::new(BucketResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureBucketInput {
    pub bucket_name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureBucketOutput {
    pub bucket_name: String,
    pub region: String,
}

#[derive(Clone)]
pub struct BucketResource {
    source: AwsClientSource,
}

impl BucketResource {
    pub fn new(source: AwsClientSource) -> Self {
        Self { source }
    }
}

fn to_output(bucket: Bucket) -> EnsureBucketOutput {
    EnsureBucketOutput {
        bucket_name: bucket.name,
        region: bucket.region,
    }
}

#[async_trait]
impl Resource for BucketResource {
    type Spec = EnsureBucketInput;
    type State = EnsureBucketOutput;

    fn kind(&self) -> &'static str {
        ENSURE_BUCKET
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let exists = client
            .s3_bucket_exists(&spec.bucket_name)
            .await
            .map_err(ResourceError::provider)?;
        if exists {
            Ok(Some(to_output(Bucket {
                name: spec.bucket_name.clone(),
                region: client.config().region.clone(),
            })))
        } else {
            Ok(None)
        }
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let created = client
            .s3_bucket_create(&spec.bucket_name)
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(created))
    }
}
