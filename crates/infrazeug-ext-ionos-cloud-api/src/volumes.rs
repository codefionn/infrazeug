//! Volume management (`/datacenters/{id}/volumes`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};

/// Volume resource returned by the Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ElementMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<VolumeProperties>,
}

/// Volume properties (read model).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VolumeProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licence_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_order: Option<String>,
}

/// Payload for creating a volume.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VolumeCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ElementMetadata>,
    pub properties: VolumeCreateProperties,
}

/// Properties accepted when creating a volume.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VolumeCreateProperties {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub size: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licence_type: Option<String>,
}

/// Payload for updating a volume.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VolumeUpdate {
    pub properties: VolumeUpdateProperties,
}

/// Properties accepted when updating a volume.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VolumeUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bus: Option<String>,
}

fn volume_path(client: &IonosClient, datacenter_id: &str, volume_id: Option<&str>) -> String {
    let mut path = format!("/datacenters/{}/volumes", client.encode_path(datacenter_id));
    if let Some(volume_id) = volume_id {
        path.push('/');
        path.push_str(&client.encode_path(volume_id));
    }
    path
}

impl IonosClient {
    /// `GET /datacenters/{dc}/volumes` — list volumes in a data center.
    pub async fn volumes(
        &self,
        datacenter_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<Volume>> {
        self.get(&volume_path(self, datacenter_id, None), query)
            .await
    }

    /// `GET /datacenters/{dc}/volumes/{id}` — retrieve one volume.
    pub async fn volume(
        &self,
        datacenter_id: &str,
        volume_id: &str,
        query: &ListQuery,
    ) -> Result<Volume> {
        self.get(&volume_path(self, datacenter_id, Some(volume_id)), query)
            .await
    }

    /// `POST /datacenters/{dc}/volumes` — create a volume.
    pub async fn create_volume(
        &self,
        datacenter_id: &str,
        body: &VolumeCreate,
        query: &ListQuery,
    ) -> Result<Volume> {
        self.post_json(&volume_path(self, datacenter_id, None), body, query)
            .await
    }

    /// `PUT /datacenters/{dc}/volumes/{id}` — replace a volume.
    pub async fn update_volume(
        &self,
        datacenter_id: &str,
        volume_id: &str,
        body: &VolumeUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<Volume> {
        self.put_json(
            &volume_path(self, datacenter_id, Some(volume_id)),
            body,
            query,
            etag,
        )
        .await
    }

    /// `DELETE /datacenters/{dc}/volumes/{id}` — delete a volume.
    pub async fn delete_volume(
        &self,
        datacenter_id: &str,
        volume_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &volume_path(self, datacenter_id, Some(volume_id)),
            query,
            etag,
        )
        .await
    }

    /// `POST /datacenters/{dc}/volumes/{id}/create-snapshot` — snapshot a volume.
    pub async fn create_volume_snapshot(
        &self,
        datacenter_id: &str,
        volume_id: &str,
        name: &str,
        query: &ListQuery,
    ) -> Result<crate::snapshots::Snapshot> {
        let body = serde_json::json!({
            "properties": { "name": name }
        });
        self.post_json(
            &format!(
                "{}/create-snapshot",
                volume_path(self, datacenter_id, Some(volume_id))
            ),
            &body,
            query,
        )
        .await
    }

    /// `POST /datacenters/{dc}/volumes/{id}/restore-snapshot` — restore from snapshot.
    pub async fn restore_volume_snapshot(
        &self,
        datacenter_id: &str,
        volume_id: &str,
        snapshot_id: &str,
        query: &ListQuery,
    ) -> Result<()> {
        let body = serde_json::json!({
            "properties": { "snapshotId": snapshot_id }
        });
        self.post(
            &format!(
                "{}/restore-snapshot",
                volume_path(self, datacenter_id, Some(volume_id))
            ),
            &body,
            query,
        )
        .await
    }
}
