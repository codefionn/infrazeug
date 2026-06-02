//! Ensure a STACKIT IaaS server exists (captured outputs only).

use crate::client::StackitClientSource;
use async_trait::async_trait;
use infrazeug_ext_stackit_api::servers::{Server, ServerBootVolume, ServerCreate};
use infrazeug_ext_stackit_api::types::ResourceSource;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_SERVER: &str = "stackit.ensure_server";

pub type EnsureServer = EnsureResource<ServerResource>;

pub fn ensure_server(source: StackitClientSource) -> EnsureServer {
    EnsureResource::new(ServerResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureServerInput {
    pub project_id: String,
    pub name: String,
    pub machine_type: String,
    /// Boot volume ID when `boot_volume_source_type` is `volume`.
    pub boot_volume_id: String,
    #[serde(default = "default_boot_volume_source_type")]
    pub boot_volume_source_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keypair_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security_groups: Option<Vec<String>>,
    /// When set, use the v2 regional API (`/v2/projects/.../regions/{region}/servers`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
}

fn default_boot_volume_source_type() -> String {
    "volume".into()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureServerOutput {
    pub server_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Clone)]
pub struct ServerResource {
    source: StackitClientSource,
}

impl ServerResource {
    pub fn new(source: StackitClientSource) -> Self {
        Self { source }
    }
}

fn to_output(server: Server) -> Option<EnsureServerOutput> {
    let id = server.id?;
    Some(EnsureServerOutput {
        server_id: id,
        name: server.name.unwrap_or_default(),
        status: server.status,
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
        let servers = if let Some(region) = &spec.region {
            client
                .servers_v2(&spec.project_id, region)
                .await
                .map_err(ResourceError::provider)?
        } else {
            client
                .servers(&spec.project_id)
                .await
                .map_err(ResourceError::provider)?
        };
        Ok(servers
            .items
            .into_iter()
            .find(|s| s.name.as_deref() == Some(spec.name.as_str()))
            .and_then(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let body = ServerCreate {
            name: spec.name.clone(),
            machine_type: spec.machine_type.clone(),
            availability_zone: spec.availability_zone.clone(),
            boot_volume: Some(ServerBootVolume {
                id: None,
                source: Some(ResourceSource {
                    id: spec.boot_volume_id.clone(),
                    source_type: spec.boot_volume_source_type.clone(),
                }),
            }),
            keypair_name: spec.keypair_name.clone(),
            network_id: spec.network_id.clone(),
            security_groups: spec.security_groups.clone(),
        };
        let created = if let Some(region) = &spec.region {
            client
                .create_server_v2(&spec.project_id, region, &body)
                .await
                .map_err(ResourceError::provider)?
        } else {
            client
                .create_server(&spec.project_id, &body)
                .await
                .map_err(ResourceError::provider)?
        };
        to_output(created).ok_or_else(|| ResourceError::provider("created server has no id"))
    }
}
