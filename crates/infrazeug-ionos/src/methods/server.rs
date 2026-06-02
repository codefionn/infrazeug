//! Ensure an IONOS Cloud server exists (captured outputs only).

use crate::client::IonosClientSource;
use async_trait::async_trait;
use infrazeug_ext_ionos_cloud_api::servers::{Server, ServerCreate, ServerCreateProperties};
use infrazeug_ext_ionos_cloud_api::ListQuery;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_SERVER: &str = "ionos.ensure_server";

/// Tier-1 method: ensure an IONOS server in a data center.
pub type EnsureServer = EnsureResource<ServerResource>;

/// Construct the registrable [`EnsureServer`] method for a client source.
pub fn ensure_server(source: IonosClientSource) -> EnsureServer {
    EnsureResource::new(ServerResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureServerInput {
    pub datacenter_id: String,
    pub name: String,
    pub cores: u32,
    pub ram: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_family: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureServerOutput {
    pub server_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_state: Option<String>,
}

/// IONOS Cloud server as an acquirable resource.
#[derive(Clone)]
pub struct ServerResource {
    source: IonosClientSource,
}

impl ServerResource {
    pub fn new(source: IonosClientSource) -> Self {
        Self { source }
    }
}

fn to_output(server: Server) -> Option<EnsureServerOutput> {
    let id = server.id?;
    let props = server.properties.unwrap_or_default();
    Some(EnsureServerOutput {
        server_id: id,
        name: props.name.unwrap_or_default(),
        vm_state: props.vm_state,
    })
}

#[async_trait]
impl Resource for ServerResource {
    type Spec = EnsureServerInput;
    type State = EnsureServerOutput;

    fn kind(&self) -> &'static str {
        ENSURE_SERVER
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let servers = client
            .servers(&spec.datacenter_id, &ListQuery::default())
            .await
            .map_err(ResourceError::provider)?;
        Ok(servers
            .items
            .into_iter()
            .find(|s| {
                s.properties.as_ref().and_then(|p| p.name.as_deref()) == Some(spec.name.as_str())
            })
            .and_then(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let body = ServerCreate {
            properties: ServerCreateProperties {
                name: spec.name.clone(),
                cores: Some(spec.cores),
                ram: Some(spec.ram),
                availability_zone: spec.availability_zone.clone(),
                cpu_family: spec.cpu_family.clone(),
                ..Default::default()
            },
            ..Default::default()
        };
        let created = client
            .create_server(&spec.datacenter_id, &body, &ListQuery::default())
            .await
            .map_err(ResourceError::provider)?;
        to_output(created).ok_or_else(|| ResourceError::provider("created server has no id"))
    }

    // `diff` stays `InSync`: cores/RAM resizing is a separate lifecycle action;
    // name + datacenter form identity here.
}
