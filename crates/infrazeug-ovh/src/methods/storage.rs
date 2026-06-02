//! Ensure an object-storage container (S3 bucket) exists.

use crate::client::OvhClientSource;
use async_trait::async_trait;
use infrazeug_ext_ovh_api::public_cloud::StorageContainerCreate;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_STORAGE_CONTAINER: &str = "ovh.ensure_storage_container";

/// Tier-1 method: ensure an OVH object-storage container.
pub type EnsureStorageContainer = EnsureResource<StorageContainerResource>;

/// Construct the registrable [`EnsureStorageContainer`] method for a client source.
pub fn ensure_storage_container(source: OvhClientSource) -> EnsureStorageContainer {
    EnsureResource::new(StorageContainerResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureStorageContainerInput {
    pub project_id: String,
    pub container_name: String,
    /// OVH region code carried in the S3 storage path (e.g. `DE`, `GRA`).
    pub region: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureStorageContainerOutput {
    /// S3 buckets have no separate id; this mirrors `name` for compatibility.
    pub container_id: String,
    pub name: String,
    pub region: String,
}

/// OVH Public Cloud object-storage container as an acquirable resource.
#[derive(Clone)]
pub struct StorageContainerResource {
    source: OvhClientSource,
}

impl StorageContainerResource {
    pub fn new(source: OvhClientSource) -> Self {
        Self { source }
    }
}

#[async_trait]
impl Resource for StorageContainerResource {
    type Spec = EnsureStorageContainerInput;
    type State = EnsureStorageContainerOutput;

    fn kind(&self) -> &'static str {
        ENSURE_STORAGE_CONTAINER
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        // The S3 storage API is region-scoped (region is in the path), so the
        // listing already only contains this region's buckets; identity is the name.
        let existing = client
            .cloud_storage_containers(&spec.project_id, &spec.region)
            .await
            .map_err(ResourceError::provider)?;
        Ok(existing
            .into_iter()
            .find(|c| c.name == spec.container_name)
            .map(|c| EnsureStorageContainerOutput {
                // S3 buckets have no separate id — the name is the identity.
                container_id: c.name.clone(),
                name: c.name,
                region: c.region.unwrap_or_else(|| spec.region.clone()),
            }))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let created = client
            .cloud_storage_container_create(
                &spec.project_id,
                &spec.region,
                &StorageContainerCreate {
                    name: spec.container_name.clone(),
                    owner_id: None,
                },
            )
            .await
            .map_err(ResourceError::provider)?;
        Ok(EnsureStorageContainerOutput {
            container_id: created.name.clone(),
            name: created.name,
            region: created.region.unwrap_or_else(|| spec.region.clone()),
        })
    }

    // `diff` stays `InSync`: name and region are the bucket's immutable identity and
    // the ensure input models nothing else mutable, so an existing bucket is never
    // reconciled. Reconcile lands when the input grows a mutable attribute.
}
