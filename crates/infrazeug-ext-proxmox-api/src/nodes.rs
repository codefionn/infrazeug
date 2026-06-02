//! Cluster node listing (`/nodes`) and version probe (`/version`).

use crate::client::ProxmoxClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// A node in the Proxmox cluster as returned by `GET /nodes`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct NodeEntry {
    pub node: String,
    /// `online` or `offline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<u64>,
}

/// Version information from `GET /version`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Version {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
}

impl ProxmoxClient {
    /// `GET /nodes` — list cluster nodes.
    pub async fn nodes(&self) -> Result<Vec<NodeEntry>> {
        self.get("/nodes").await
    }

    /// `GET /version` — probe the API version (useful as a connectivity check).
    pub async fn version(&self) -> Result<Version> {
        self.get("/version").await
    }
}
