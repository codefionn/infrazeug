//! API v2 asynchronous tasks (`…/task` under domain resources).

use crate::client::{OvhClient, V2PageInfo, V2RequestOptions};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Global status of an asynchronous task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    Done,
    Error,
    Pending,
    Running,
    Scheduled,
    WaitingUserInput,
}

/// One progress step on a task.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub name: String,
    pub status: TaskStatus,
}

/// Error reported on a failed task.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskError {
    pub message: String,
}

/// Asynchronous operation (`common.Task`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2Task {
    pub id: String,
    pub status: TaskStatus,
    pub message: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub link: String,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub errors: Option<Vec<TaskError>>,
    #[serde(default)]
    pub progress: Vec<TaskProgress>,
}

/// Paginated task list wrapper.
#[derive(Debug, Clone)]
pub struct V2TaskList {
    pub items: Vec<V2Task>,
    pub page: V2PageInfo,
}

/// Additional details for `PUT …/task/{taskId}` on domain names.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainTaskAdditionalDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dnssec_risk_acknowledged: Option<bool>,
}

impl OvhClient {
    /// `GET /domain/alldom/{alldomName}/task` — list tasks for an AllDom resource.
    pub async fn domain_alldom_tasks(
        &self,
        alldom_name: &str,
        options: V2RequestOptions<'_>,
    ) -> Result<V2TaskList> {
        let path = format!("/domain/alldom/{}/task", self.encode_segment(alldom_name));
        let (items, page) = self.get_v2(&path, options).await?;
        Ok(V2TaskList { items, page })
    }

    /// `GET /domain/alldom/{alldomName}/task/{taskId}` — fetch one AllDom task.
    pub async fn domain_alldom_task(&self, alldom_name: &str, task_id: &str) -> Result<V2Task> {
        let path = format!(
            "/domain/alldom/{}/task/{}",
            self.encode_segment(alldom_name),
            self.encode_segment(task_id),
        );
        let (task, _) = self.get_v2(&path, V2RequestOptions::default()).await?;
        Ok(task)
    }

    /// `GET /domain/name/{domainName}/task` — list tasks for a domain name.
    pub async fn domain_name_tasks(
        &self,
        domain_name: &str,
        options: V2RequestOptions<'_>,
    ) -> Result<V2TaskList> {
        let path = format!("/domain/name/{}/task", self.encode_segment(domain_name));
        let (items, page) = self.get_v2(&path, options).await?;
        Ok(V2TaskList { items, page })
    }

    /// `GET /domain/name/{domainName}/task/{taskId}` — fetch one domain task.
    pub async fn domain_name_task(&self, domain_name: &str, task_id: &str) -> Result<V2Task> {
        let path = format!(
            "/domain/name/{}/task/{}",
            self.encode_segment(domain_name),
            self.encode_segment(task_id),
        );
        let (task, _) = self.get_v2(&path, V2RequestOptions::default()).await?;
        Ok(task)
    }

    /// `PUT /domain/name/{domainName}/task/{taskId}` — supply additional task details.
    pub async fn domain_name_task_update(
        &self,
        domain_name: &str,
        task_id: &str,
        details: &DomainTaskAdditionalDetails,
        options: V2RequestOptions<'_>,
    ) -> Result<V2Task> {
        let path = format!(
            "/domain/name/{}/task/{}",
            self.encode_segment(domain_name),
            self.encode_segment(task_id),
        );
        self.put_v2(&path, details, options).await
    }
}
