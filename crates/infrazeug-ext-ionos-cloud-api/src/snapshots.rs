//! Snapshot management (`/snapshots`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};

/// Snapshot resource returned by the Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
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
    pub properties: Option<SnapshotProperties>,
}

/// Snapshot properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licence_type: Option<String>,
}

/// Payload for updating a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotUpdate {
    pub properties: SnapshotUpdateProperties,
}

/// Properties accepted when updating a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub licence_type: Option<String>,
}

impl IonosClient {
    /// `GET /snapshots` — list snapshots.
    pub async fn snapshots(&self, query: &ListQuery) -> Result<Collection<Snapshot>> {
        self.get("/snapshots", query).await
    }

    /// `GET /snapshots/{id}` — retrieve one snapshot.
    pub async fn snapshot(&self, snapshot_id: &str, query: &ListQuery) -> Result<Snapshot> {
        self.get(
            &format!("/snapshots/{}", self.encode_path(snapshot_id)),
            query,
        )
        .await
    }

    /// `PUT /snapshots/{id}` — update a snapshot.
    pub async fn update_snapshot(
        &self,
        snapshot_id: &str,
        body: &SnapshotUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<Snapshot> {
        self.put_json(
            &format!("/snapshots/{}", self.encode_path(snapshot_id)),
            body,
            query,
            etag,
        )
        .await
    }

    /// `DELETE /snapshots/{id}` — delete a snapshot.
    pub async fn delete_snapshot(
        &self,
        snapshot_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &format!("/snapshots/{}", self.encode_path(snapshot_id)),
            query,
            etag,
        )
        .await
    }
}
