//! Ensure an R2 bucket exists (create or reconcile storage class).

use crate::client::CloudflareClientSource;
use crate::methods::account::resolve_account_id;
use async_trait::async_trait;
use infrazeug_ext_cloudflare_api::r2_bucket::{R2Bucket, R2BucketCreate};
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_R2_BUCKET: &str = "cloudflare.ensure_r2_bucket";

/// Tier-1 method: ensure an R2 bucket exists.
pub type EnsureR2Bucket = EnsureResource<R2BucketResource>;

/// Construct the registrable [`EnsureR2Bucket`] method for a client source.
pub fn ensure_r2_bucket(source: CloudflareClientSource) -> EnsureR2Bucket {
    EnsureResource::new(R2BucketResource::new(source))
}

/// Desired R2 bucket. Natural key: account + `name`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureR2BucketInput {
    /// Account id (32-char hex). Provide this, [`account_name`](Self::account_name), or set `CLOUDFLARE_ACCOUNT_ID`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// Account display name (resolved via `GET /accounts?name=…`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_name: Option<String>,
    /// Globally unique bucket name.
    pub name: String,
    /// Region hint on create (`wnam`, `enam`, `weur`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_hint: Option<String>,
    /// `Standard` (default) or `InfrequentAccess`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<String>,
    /// Jurisdiction header for EU/FedRAMP buckets (`default`, `eu`, `fedramp`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
}

/// Observed R2 bucket — managed fields from the API.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureR2BucketOutput {
    pub account_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation_date: Option<String>,
}

#[derive(Clone)]
pub struct R2BucketResource {
    source: CloudflareClientSource,
}

impl R2BucketResource {
    pub fn new(source: CloudflareClientSource) -> Self {
        Self { source }
    }

    async fn find(
        &self,
        ctx: &ResourceCtx,
        account_id: &str,
        spec: &EnsureR2BucketInput,
    ) -> ResourceResult<Option<R2Bucket>> {
        let client = self.source.client(ctx).await?;
        client
            .try_r2_bucket(account_id, &spec.name, spec.jurisdiction.as_deref())
            .await
            .map_err(ResourceError::provider)
    }
}

fn to_output(account_id: &str, bucket: R2Bucket) -> EnsureR2BucketOutput {
    EnsureR2BucketOutput {
        account_id: account_id.to_string(),
        name: bucket.name.unwrap_or_default(),
        location: bucket.location,
        storage_class: bucket.storage_class,
        jurisdiction: bucket.jurisdiction,
        creation_date: bucket.creation_date,
    }
}

fn build_create(spec: &EnsureR2BucketInput) -> R2BucketCreate {
    R2BucketCreate {
        name: spec.name.clone(),
        location_hint: spec.location_hint.clone(),
        storage_class: spec.storage_class.clone(),
    }
}

#[async_trait]
impl Resource for R2BucketResource {
    type Spec = EnsureR2BucketInput;
    type State = EnsureR2BucketOutput;

    fn kind(&self) -> &'static str {
        ENSURE_R2_BUCKET
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let account_id =
            resolve_account_id(&self.source, ctx, &spec.account_id, &spec.account_name).await?;
        Ok(self
            .find(ctx, &account_id, spec)
            .await?
            .map(|b| to_output(&account_id, b)))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let account_id =
            resolve_account_id(&self.source, ctx, &spec.account_id, &spec.account_name).await?;
        let client = self.source.client(ctx).await?;
        let created = client
            .create_r2_bucket(
                &account_id,
                &build_create(spec),
                spec.jurisdiction.as_deref(),
            )
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(&account_id, created))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if let Some(ref storage_class) = spec.storage_class {
            if current.storage_class.as_deref() != Some(storage_class.as_str()) {
                diffs.push(format!(
                    "storage_class {:?} → {}",
                    current.storage_class, storage_class
                ));
            }
        }
        if diffs.is_empty() {
            Drift::InSync
        } else {
            Drift::Drifted(diffs.join(", "))
        }
    }

    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let account_id =
            resolve_account_id(&self.source, ctx, &spec.account_id, &spec.account_name).await?;
        let storage_class = spec.storage_class.as_deref().ok_or_else(|| {
            ResourceError::provider("storage_class required to reconcile r2 bucket drift")
        })?;
        let client = self.source.client(ctx).await?;
        let updated = client
            .patch_r2_bucket_storage_class(
                &account_id,
                &current.name,
                storage_class,
                spec.jurisdiction.as_deref(),
            )
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(&account_id, updated))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> R2BucketResource {
        R2BucketResource::new(CloudflareClientSource::vault("cloud/cloudflare.vault"))
    }

    fn current() -> EnsureR2BucketOutput {
        EnsureR2BucketOutput {
            account_id: "acc123".into(),
            name: "logs".into(),
            location: Some("wnam".into()),
            storage_class: Some("Standard".into()),
            jurisdiction: None,
            creation_date: None,
        }
    }

    #[test]
    fn matching_spec_is_in_sync() {
        let spec = EnsureR2BucketInput {
            account_id: Some("acc123".into()),
            name: "logs".into(),
            storage_class: Some("Standard".into()),
            ..Default::default()
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_storage_class_drifts() {
        let spec = EnsureR2BucketInput {
            account_id: Some("acc123".into()),
            name: "logs".into(),
            storage_class: Some("InfrequentAccess".into()),
            ..Default::default()
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }
}
