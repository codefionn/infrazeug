//! Public Cloud project metadata (`/cloud/project`).

use crate::client::OvhClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Public Cloud project status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Creating,
    Deleted,
    Deleting,
    Ok,
    Suspended,
    #[serde(other)]
    Unknown,
}

/// A Public Cloud project (`cloud.Project`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudProject {
    pub project_id: String,
    pub description: Option<String>,
    pub project_name: Option<String>,
    pub status: ProjectStatus,
    pub plan_code: String,
    pub creation_date: String,
    pub expiration: Option<String>,
    pub manual_quota: bool,
    pub order_id: Option<i64>,
}

impl OvhClient {
    /// `GET /cloud/project` — list Public Cloud project ids.
    pub async fn cloud_projects(&self) -> Result<Vec<String>> {
        self.get_v1("/cloud/project").await
    }

    /// `GET /cloud/project/{serviceName}` — project details.
    pub async fn cloud_project(&self, service_name: &str) -> Result<CloudProject> {
        let path = super::project_path(service_name, self, "");
        self.get_v1(&path).await
    }
}
