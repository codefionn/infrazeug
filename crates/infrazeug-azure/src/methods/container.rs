use crate::client::AzureClientSource;
use async_trait::async_trait;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_CONTAINER: &str = "azure.ensure_container";

pub type EnsureContainer = EnsureResource<ContainerResource>;

pub fn ensure_container(source: AzureClientSource) -> EnsureContainer {
    EnsureResource::new(ContainerResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureContainerInput {
    pub storage_account: String,
    pub container_name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureContainerOutput {
    pub storage_account: String,
    pub container_name: String,
}

#[derive(Clone)]
pub struct ContainerResource {
    source: AzureClientSource,
}

impl ContainerResource {
    pub fn new(source: AzureClientSource) -> Self {
        Self { source }
    }
}

#[async_trait]
impl Resource for ContainerResource {
    type Spec = EnsureContainerInput;
    type State = EnsureContainerOutput;

    fn kind(&self) -> &'static str {
        ENSURE_CONTAINER
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let exists = client
            .blob_container_exists(&spec.storage_account, &spec.container_name)
            .await
            .map_err(ResourceError::provider)?;
        if exists {
            Ok(Some(EnsureContainerOutput {
                storage_account: spec.storage_account.clone(),
                container_name: spec.container_name.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        client
            .blob_container_create(&spec.storage_account, &spec.container_name)
            .await
            .map_err(ResourceError::provider)?;
        Ok(EnsureContainerOutput {
            storage_account: spec.storage_account.clone(),
            container_name: spec.container_name.clone(),
        })
    }
}
