//! Ensure a B2 bucket exists (create or reconcile bucket type).

use crate::client::BackblazeClientSource;
use async_trait::async_trait;
use infrazeug_ext_backblaze_api::bucket::{Bucket, BucketCreate, BucketUpdate};
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ENSURE_BUCKET: &str = "backblaze.ensure_bucket";

pub type EnsureBucket = EnsureResource<BucketResource>;

pub fn ensure_bucket(source: BackblazeClientSource) -> EnsureBucket {
    EnsureResource::new(BucketResource::new(source))
}

/// Desired B2 bucket. Natural key: `bucket_name`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureBucketInput {
    /// Globally unique bucket name (6–63 characters).
    pub bucket_name: String,
    /// `allPrivate` (default) or `allPublic`.
    #[serde(default = "default_bucket_type")]
    pub bucket_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_info: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cors_rules: Option<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_rules: Option<Vec<Value>>,
}

fn default_bucket_type() -> String {
    "allPrivate".into()
}

/// Observed B2 bucket — managed fields from the API.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureBucketOutput {
    pub account_id: String,
    pub bucket_id: String,
    pub bucket_name: String,
    pub bucket_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_info: Option<Value>,
}

#[derive(Clone)]
pub struct BucketResource {
    source: BackblazeClientSource,
}

impl BucketResource {
    pub fn new(source: BackblazeClientSource) -> Self {
        Self { source }
    }
}

fn to_output(bucket: Bucket) -> EnsureBucketOutput {
    EnsureBucketOutput {
        account_id: bucket.account_id.unwrap_or_default(),
        bucket_id: bucket.bucket_id.unwrap_or_default(),
        bucket_name: bucket.bucket_name.unwrap_or_default(),
        bucket_type: bucket.bucket_type.unwrap_or_else(|| "allPrivate".into()),
        bucket_info: bucket.bucket_info,
    }
}

fn build_create(account_id: &str, spec: &EnsureBucketInput) -> BucketCreate {
    BucketCreate {
        account_id: account_id.into(),
        bucket_name: spec.bucket_name.clone(),
        bucket_type: spec.bucket_type.clone(),
        bucket_info: spec.bucket_info.clone(),
        cors_rules: spec.cors_rules.clone(),
        lifecycle_rules: spec.lifecycle_rules.clone(),
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
        Ok(client
            .try_bucket(&spec.bucket_name)
            .await
            .map_err(ResourceError::provider)?
            .map(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let account_id = client.account_id().await.map_err(ResourceError::provider)?;
        let created = client
            .create_bucket(&build_create(&account_id, spec))
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(created))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        if current.bucket_type != spec.bucket_type {
            Drift::Drifted(format!(
                "bucket_type {:?} → {}",
                current.bucket_type, spec.bucket_type
            ))
        } else {
            Drift::InSync
        }
    }

    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let account_id = client.account_id().await.map_err(ResourceError::provider)?;
        let updated = client
            .update_bucket(&BucketUpdate {
                account_id,
                bucket_id: current.bucket_id,
                bucket_type: Some(spec.bucket_type.clone()),
                bucket_info: spec.bucket_info.clone(),
                cors_rules: spec.cors_rules.clone(),
                lifecycle_rules: spec.lifecycle_rules.clone(),
            })
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(updated))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> BucketResource {
        BucketResource::new(BackblazeClientSource::vault("cloud/backblaze.vault"))
    }

    fn current() -> EnsureBucketOutput {
        EnsureBucketOutput {
            account_id: "acc".into(),
            bucket_id: "bid".into(),
            bucket_name: "logs".into(),
            bucket_type: "allPrivate".into(),
            bucket_info: None,
        }
    }

    #[test]
    fn matching_spec_is_in_sync() {
        let spec = EnsureBucketInput {
            bucket_name: "logs".into(),
            bucket_type: "allPrivate".into(),
            ..Default::default()
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_bucket_type_drifts() {
        let spec = EnsureBucketInput {
            bucket_name: "logs".into(),
            bucket_type: "allPublic".into(),
            ..Default::default()
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }
}
