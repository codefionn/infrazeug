//! Volume management (`/projects/{id}/volumes`).

use crate::client::StackitClient;
use crate::error::Result;
use crate::types::{ItemList, ResourceSource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Block volume resource returned by the IaaS API.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ResourceSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,
}

/// Payload for creating a volume.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VolumeCreate {
    pub name: String,
    pub size: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ResourceSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance_class: Option<String>,
}

impl StackitClient {
    /// `GET /v1/projects/{projectId}/volumes` — list volumes in a project.
    pub async fn volumes(&self, project_id: &str) -> Result<ItemList<Volume>> {
        self.get(&self.project_path(project_id, "volumes")).await
    }

    /// `GET /v1/projects/{projectId}/volumes/{id}` — retrieve one volume.
    pub async fn volume(&self, project_id: &str, volume_id: &str) -> Result<Volume> {
        self.get(&format!(
            "{}/{}",
            self.project_path(project_id, "volumes"),
            self.encode_path(volume_id)
        ))
        .await
    }

    /// `POST /v1/projects/{projectId}/volumes` — create a volume.
    pub async fn create_volume(&self, project_id: &str, body: &VolumeCreate) -> Result<Volume> {
        self.post_json(&self.project_path(project_id, "volumes"), body)
            .await
    }

    /// `GET /v2/projects/{projectId}/regions/{region}/volumes` — list volumes (v2).
    pub async fn volumes_v2(&self, project_id: &str, region: &str) -> Result<ItemList<Volume>> {
        self.get(&self.regional_path(project_id, region, "volumes"))
            .await
    }

    /// `POST /v2/projects/{projectId}/regions/{region}/volumes` — create a volume (v2).
    pub async fn create_volume_v2(
        &self,
        project_id: &str,
        region: &str,
        body: &VolumeCreate,
    ) -> Result<Volume> {
        self.post_json(&self.regional_path(project_id, region, "volumes"), body)
            .await
    }

    /// `DELETE /v1/projects/{projectId}/volumes/{id}` — delete a volume.
    pub async fn delete_volume(&self, project_id: &str, volume_id: &str) -> Result<()> {
        self.delete(&format!(
            "{}/{}",
            self.project_path(project_id, "volumes"),
            self.encode_path(volume_id)
        ))
        .await
    }

    /// `DELETE /v2/projects/{projectId}/regions/{region}/volumes/{id}` — delete a volume (v2).
    pub async fn delete_volume_v2(
        &self,
        project_id: &str,
        region: &str,
        volume_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "{}/{}",
            self.regional_path(project_id, region, "volumes"),
            self.encode_path(volume_id)
        ))
        .await
    }
}
