//! Ensure a GCS bucket exists.

use crate::client::GcpClientSource;
use async_trait::async_trait;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_BUCKET: &str = "gcp.ensure_bucket";

pub type EnsureBucket = EnsureResource<BucketResource>;

pub fn ensure_bucket(source: GcpClientSource) -> EnsureBucket {
    EnsureResource::new(BucketResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureBucketInput {
    pub bucket_name: String,
    pub location: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureBucketOutput {
    pub bucket_name: String,
    pub location: String,
}

#[derive(Clone)]
pub struct BucketResource {
    source: GcpClientSource,
}

impl BucketResource {
    pub fn new(source: GcpClientSource) -> Self {
        Self { source }
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
            .storage_bucket_exists(&spec.bucket_name)
            .await
            .map_err(ResourceError::provider)?;
        if exists {
            Ok(Some(EnsureBucketOutput {
                bucket_name: spec.bucket_name.clone(),
                location: spec.location.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let created = client
            .storage_bucket_create(&spec.bucket_name, &spec.location)
            .await
            .map_err(ResourceError::provider)?;
        Ok(EnsureBucketOutput {
            bucket_name: created.name,
            location: created.location,
        })
    }
}
