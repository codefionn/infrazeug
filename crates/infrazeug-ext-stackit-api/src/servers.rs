//! Server management (`/projects/{id}/servers`).

use crate::client::StackitClient;
use crate::error::Result;
use crate::types::{ItemList, ResourceSource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Server resource returned by the IaaS API.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_volume: Option<ServerBootVolume>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// Boot volume reference on a server.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerBootVolume {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ResourceSource>,
}

/// Payload for creating a server.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerCreate {
    pub name: String,
    pub machine_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_volume: Option<ServerBootVolume>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keypair_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_groups: Option<Vec<String>>,
}

impl StackitClient {
    /// `GET /v1/projects/{projectId}/servers` — list servers in a project.
    pub async fn servers(&self, project_id: &str) -> Result<ItemList<Server>> {
        self.get(&self.project_path(project_id, "servers")).await
    }

    /// `GET /v1/projects/{projectId}/servers/{id}` — retrieve one server.
    pub async fn server(&self, project_id: &str, server_id: &str) -> Result<Server> {
        self.get(&format!(
            "{}/{}",
            self.project_path(project_id, "servers"),
            self.encode_path(server_id)
        ))
        .await
    }

    /// `POST /v1/projects/{projectId}/servers` — create a server.
    pub async fn create_server(&self, project_id: &str, body: &ServerCreate) -> Result<Server> {
        self.post_json(&self.project_path(project_id, "servers"), body)
            .await
    }

    /// `GET /v2/projects/{projectId}/regions/{region}/servers` — list servers (v2).
    pub async fn servers_v2(&self, project_id: &str, region: &str) -> Result<ItemList<Server>> {
        self.get(&self.regional_path(project_id, region, "servers"))
            .await
    }

    /// `POST /v2/projects/{projectId}/regions/{region}/servers` — create a server (v2).
    pub async fn create_server_v2(
        &self,
        project_id: &str,
        region: &str,
        body: &ServerCreate,
    ) -> Result<Server> {
        self.post_json(&self.regional_path(project_id, region, "servers"), body)
            .await
    }

    /// `DELETE /v1/projects/{projectId}/servers/{id}` — delete a server.
    pub async fn delete_server(&self, project_id: &str, server_id: &str) -> Result<()> {
        self.delete(&format!(
            "{}/{}",
            self.project_path(project_id, "servers"),
            self.encode_path(server_id)
        ))
        .await
    }

    /// `DELETE /v2/projects/{projectId}/regions/{region}/servers/{id}` — delete a server (v2).
    pub async fn delete_server_v2(
        &self,
        project_id: &str,
        region: &str,
        server_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "{}/{}",
            self.regional_path(project_id, region, "servers"),
            self.encode_path(server_id)
        ))
        .await
    }
}
