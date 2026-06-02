//! Ensure an IONOS Cloud data center exists (captured outputs only).

use crate::client::IonosClientSource;
use async_trait::async_trait;
use infrazeug_ext_ionos_cloud_api::datacenters::{
    Datacenter, DatacenterCreate, DatacenterCreateProperties,
};
use infrazeug_ext_ionos_cloud_api::ListQuery;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_DATACENTER: &str = "ionos.ensure_datacenter";

pub type EnsureDatacenter = EnsureResource<DatacenterResource>;

pub fn ensure_datacenter(source: IonosClientSource) -> EnsureDatacenter {
    EnsureResource::new(DatacenterResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureDatacenterInput {
    pub name: String,
    /// IONOS location slug, e.g. `de/fra` or `us/las`.
    pub location: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureDatacenterOutput {
    pub datacenter_id: String,
    pub name: String,
    pub location: String,
}

#[derive(Clone)]
pub struct DatacenterResource {
    source: IonosClientSource,
}

impl DatacenterResource {
    pub fn new(source: IonosClientSource) -> Self {
        Self { source }
    }
}

fn to_output(dc: Datacenter) -> Option<EnsureDatacenterOutput> {
    let id = dc.id?;
    let props = dc.properties.unwrap_or_default();
    Some(EnsureDatacenterOutput {
        datacenter_id: id,
        name: props.name.unwrap_or_default(),
        location: props.location.unwrap_or_default(),
    })
}

#[async_trait]
impl Resource for DatacenterResource {
    type Spec = EnsureDatacenterInput;
    type State = EnsureDatacenterOutput;

    fn kind(&self) -> &'static str {
        ENSURE_DATACENTER
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let datacenters = client
            .datacenters(&ListQuery::default())
            .await
            .map_err(ResourceError::provider)?;
        Ok(datacenters
            .items
            .into_iter()
            .find(|dc| {
                dc.properties.as_ref().and_then(|p| p.name.as_deref()) == Some(spec.name.as_str())
            })
            .and_then(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let body = DatacenterCreate {
            properties: DatacenterCreateProperties {
                name: spec.name.clone(),
                location: spec.location.clone(),
                description: spec.description.clone(),
                ..Default::default()
            },
            ..Default::default()
        };
        let created = client
            .create_datacenter(&body, &ListQuery::default())
            .await
            .map_err(ResourceError::provider)?;
        to_output(created).ok_or_else(|| ResourceError::provider("created datacenter has no id"))
    }
}
