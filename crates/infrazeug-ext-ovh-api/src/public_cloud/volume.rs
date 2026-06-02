//! Public Cloud **block storage** — volumes and snapshots (`/cloud/project/…/volume`).

use super::project_path;
use crate::client::OvhClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Block volume performance tier (`cloud.volume.VolumeTypeEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum VolumeType {
    Classic,
    #[serde(rename = "classic-luks")]
    ClassicLuks,
    #[serde(rename = "classic-multiattach")]
    ClassicMultiattach,
    #[serde(rename = "high-speed")]
    HighSpeed,
    #[serde(rename = "high-speed-gen2")]
    HighSpeedGen2,
    #[serde(rename = "high-speed-gen2-luks")]
    HighSpeedGen2Luks,
    #[serde(rename = "high-speed-luks")]
    HighSpeedLuks,
    #[serde(other)]
    Unknown,
}

/// Optional filter for `GET …/volume`.
#[derive(Debug, Clone, Default)]
pub struct VolumeListQuery<'a> {
    pub region: Option<&'a str>,
}

/// A block volume (`cloud.volume.Volume`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub id: String,
    pub name: String,
    pub region: String,
    pub size: i64,
    pub status: String,
    pub bootable: bool,
    pub description: String,
    pub creation_date: String,
    #[serde(default)]
    pub attached_to: Vec<String>,
    #[serde(default)]
    pub availability_zone: Option<String>,
    #[serde(default)]
    pub plan_code: Option<String>,
    #[serde(default)]
    pub volume_type: Option<VolumeType>,
}

/// Body for `POST …/volume` (`cloud.ProjectVolumeCreation`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeCreate {
    pub region: String,
    pub size: i64,
    pub r#type: VolumeType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
}

/// Body for `PUT …/volume/{volumeId}` (`cloud.ProjectVolumeUpdate`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Body for `POST …/volume/{volumeId}/attach`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeAttach {
    pub instance_id: String,
}

/// Body for `POST …/volume/{volumeId}/detach`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeDetach {
    pub instance_id: String,
}

/// Body for `POST …/volume/{volumeId}/upsize`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeUpsize {
    pub size: i64,
}

/// A volume snapshot (`cloud.volume.Snapshot`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeSnapshot {
    pub id: String,
    pub name: String,
    pub region: String,
    pub size: i64,
    pub status: String,
    pub description: String,
    pub creation_date: String,
    pub volume_id: String,
    #[serde(default)]
    pub plan_code: Option<String>,
}

/// Body for `POST …/volume/{volumeId}/snapshot`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeSnapshotCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl OvhClient {
    /// `GET /cloud/project/{serviceName}/volume` — list block volumes.
    pub async fn cloud_volumes(
        &self,
        service_name: &str,
        query: VolumeListQuery<'_>,
    ) -> Result<Vec<Volume>> {
        let mut path = project_path(service_name, self, "/volume");
        if let Some(region) = query.region {
            path = Self::append_query(&path, &[("region", region)]);
        }
        self.get_v1_url(&path).await
    }

    /// `POST /cloud/project/{serviceName}/volume` — create a volume.
    pub async fn cloud_volume_create(
        &self,
        service_name: &str,
        create: &VolumeCreate,
    ) -> Result<Volume> {
        let path = project_path(service_name, self, "/volume");
        self.post_v1(&path, create).await
    }

    /// `GET /cloud/project/{serviceName}/volume/{volumeId}`.
    pub async fn cloud_volume(&self, service_name: &str, volume_id: &str) -> Result<Volume> {
        let path = volume_path(self, service_name, volume_id);
        self.get_v1(&path).await
    }

    /// `PUT /cloud/project/{serviceName}/volume/{volumeId}` — rename or update description.
    pub async fn cloud_volume_update(
        &self,
        service_name: &str,
        volume_id: &str,
        update: &VolumeUpdate,
    ) -> Result<Volume> {
        let path = volume_path(self, service_name, volume_id);
        self.put_v1_typed(&path, update).await
    }

    /// `DELETE /cloud/project/{serviceName}/volume/{volumeId}`.
    pub async fn cloud_volume_delete(&self, service_name: &str, volume_id: &str) -> Result<()> {
        let path = volume_path(self, service_name, volume_id);
        self.delete_v1(&path).await
    }

    /// `POST …/volume/{volumeId}/attach` — attach to an instance.
    pub async fn cloud_volume_attach(
        &self,
        service_name: &str,
        volume_id: &str,
        attach: &VolumeAttach,
    ) -> Result<Volume> {
        let path = format!("{}/attach", volume_path(self, service_name, volume_id));
        self.post_v1(&path, attach).await
    }

    /// `POST …/volume/{volumeId}/detach` — detach from an instance.
    pub async fn cloud_volume_detach(
        &self,
        service_name: &str,
        volume_id: &str,
        detach: &VolumeDetach,
    ) -> Result<Volume> {
        let path = format!("{}/detach", volume_path(self, service_name, volume_id));
        self.post_v1(&path, detach).await
    }

    /// `POST …/volume/{volumeId}/upsize` — grow volume size.
    pub async fn cloud_volume_upsize(
        &self,
        service_name: &str,
        volume_id: &str,
        upsize: &VolumeUpsize,
    ) -> Result<Volume> {
        let path = format!("{}/upsize", volume_path(self, service_name, volume_id));
        self.post_v1(&path, upsize).await
    }

    /// `GET /cloud/project/{serviceName}/volume/snapshot` — list volume snapshots.
    pub async fn cloud_volume_snapshots(&self, service_name: &str) -> Result<Vec<VolumeSnapshot>> {
        let path = project_path(service_name, self, "/volume/snapshot");
        self.get_v1(&path).await
    }

    /// `GET /cloud/project/{serviceName}/volume/snapshot/{snapshotId}`.
    pub async fn cloud_volume_snapshot(
        &self,
        service_name: &str,
        snapshot_id: &str,
    ) -> Result<VolumeSnapshot> {
        let path = format!(
            "{}/{}",
            project_path(service_name, self, "/volume/snapshot"),
            self.encode_segment(snapshot_id)
        );
        self.get_v1(&path).await
    }

    /// `POST …/volume/{volumeId}/snapshot` — snapshot a volume.
    pub async fn cloud_volume_snapshot_create(
        &self,
        service_name: &str,
        volume_id: &str,
        create: &VolumeSnapshotCreate,
    ) -> Result<VolumeSnapshot> {
        let path = format!("{}/snapshot", volume_path(self, service_name, volume_id));
        self.post_v1(&path, create).await
    }

    /// `DELETE /cloud/project/{serviceName}/volume/snapshot/{snapshotId}`.
    pub async fn cloud_volume_snapshot_delete(
        &self,
        service_name: &str,
        snapshot_id: &str,
    ) -> Result<()> {
        let path = format!(
            "{}/{}",
            project_path(service_name, self, "/volume/snapshot"),
            self.encode_segment(snapshot_id)
        );
        self.delete_v1(&path).await
    }
}

fn volume_path(client: &OvhClient, service_name: &str, volume_id: &str) -> String {
    format!(
        "{}/{}",
        project_path(service_name, client, "/volume"),
        client.encode_segment(volume_id)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_volume() {
        let vol: Volume = serde_json::from_str(
            r#"{
                "id": "v1",
                "name": "data",
                "region": "GRA11",
                "size": 50,
                "status": "available",
                "bootable": false,
                "description": "",
                "creationDate": "2024-01-01T00:00:00+00:00",
                "attachedTo": []
            }"#,
        )
        .unwrap();
        assert_eq!(vol.size, 50);
    }

    #[test]
    fn serializes_volume_create() {
        let body = VolumeCreate {
            region: "GRA11".into(),
            size: 10,
            r#type: VolumeType::HighSpeedGen2,
            name: Some("data".into()),
            description: None,
            image_id: None,
            snapshot_id: None,
            availability_zone: None,
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains(r#""type":"high-speed-gen2""#));
    }
}
