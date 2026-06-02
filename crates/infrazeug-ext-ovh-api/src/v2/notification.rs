//! OVHcloud API v2 **notification** bindings (`/v2/notification`).
//!
//! Generated from the official schema; do not edit by hand.

#![allow(unused_imports, unused_variables)]

use crate::client::{percent_encode, OvhClient, PageParams, V2RequestOptions};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// `common.CurrentTaskStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommonCurrentTaskStatus {
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "PENDING")]
    Pending,
    #[serde(rename = "RUNNING")]
    Running,
    #[serde(rename = "SCHEDULED")]
    Scheduled,
    #[serde(rename = "WAITING_USER_INPUT")]
    WaitingUserInput,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `common.TaskError`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskError {
    pub message: Option<String>,
}

/// `common.CurrentTask`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentTask {
    pub errors: Option<Vec<TaskError>>,
    pub id: Option<String>,
    pub link: Option<String>,
    pub status: Option<CommonCurrentTaskStatus>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

/// `common.TaskStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommonTaskStatus {
    #[serde(rename = "DONE")]
    Done,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "PENDING")]
    Pending,
    #[serde(rename = "RUNNING")]
    Running,
    #[serde(rename = "SCHEDULED")]
    Scheduled,
    #[serde(rename = "WAITING_USER_INPUT")]
    WaitingUserInput,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `common.TaskProgress`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub name: Option<String>,
    pub status: Option<CommonTaskStatus>,
}

/// `common.Task`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub created_at: Option<String>,
    pub errors: Option<Vec<TaskError>>,
    pub finished_at: Option<String>,
    pub id: Option<String>,
    pub link: Option<String>,
    pub message: Option<String>,
    pub progress: Option<Vec<TaskProgress>>,
    pub started_at: Option<String>,
    pub status: Option<CommonTaskStatus>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub updated_at: Option<String>,
}

/// `common.TaskWithInputs<T>`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskWithInputsT {
    pub created_at: Option<String>,
    pub errors: Option<Vec<TaskError>>,
    pub finished_at: Option<String>,
    pub id: Option<String>,
    #[serde(default)]
    pub inputs: serde_json::Value,
    pub link: Option<String>,
    pub message: Option<String>,
    pub progress: Option<Vec<TaskProgress>>,
    pub started_at: Option<String>,
    pub status: Option<CommonTaskStatus>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub updated_at: Option<String>,
}

/// `notification.SortOrderEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationSortOrder {
    #[serde(rename = "ASC")]
    Asc,
    #[serde(rename = "DESC")]
    Desc,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `notification.contactMean.StatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationContactMeanStatus {
    #[serde(rename = "DISABLED")]
    Disabled,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "TO_VALIDATE")]
    ToValidate,
    #[serde(rename = "VALID")]
    Valid,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `notification.contactMean.TypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationContactMeanType {
    #[serde(rename = "EMAIL")]
    Email,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `notification.contactMean.ContactMean`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactMeanContactMean {
    pub created_at: Option<String>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub default: Option<bool>,
    pub description: Option<String>,
    pub email: Option<String>,
    pub error: Option<String>,
    pub id: Option<String>,
    pub status: Option<NotificationContactMeanStatus>,
    #[serde(rename = "type")]
    pub kind: Option<NotificationContactMeanType>,
}

/// `notification.contactMean.ContactMeanTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationContactMeanContactMeanType {
    #[serde(rename = "EMAIL")]
    Email,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `notification.contactMean.PostInput`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactMeanPostInput {
    pub description: Option<String>,
    pub email: Option<String>,
    #[serde(rename = "type")]
    pub kind: NotificationContactMeanType,
}

/// `notification.contactMean.TaskInputs`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactMeanTaskInputs {
    pub otp: Option<String>,
    pub resend_otp: Option<String>,
}

/// `notification.contactMean.ValidateInput`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactMeanValidateInput {
    pub otp: String,
}

/// `notification.history.Attachment`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAttachment {
    pub content_type: Option<String>,
    #[serde(default)]
    pub name: serde_json::Value,
    pub size_bytes: Option<i64>,
    pub url: Option<String>,
}

/// `notification.history.ContactStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationHistoryContactStatus {
    #[serde(rename = "BOUNCED")]
    Bounced,
    #[serde(rename = "DELIVERED")]
    Delivered,
    #[serde(rename = "DROPPED")]
    Dropped,
    #[serde(rename = "QUEUED")]
    Queued,
    #[serde(rename = "SENT")]
    Sent,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `notification.history.Contact`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryContact {
    pub error: Option<String>,
    pub id: Option<String>,
    pub sent_at: Option<String>,
    pub status: Option<NotificationHistoryContactStatus>,
    pub to: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<NotificationContactMeanContactMeanType>,
}

/// `notification.history.NotificationPriorityEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationHistoryNotificationPriority {
    #[serde(rename = "HIGH")]
    High,
    #[serde(rename = "LOW")]
    Low,
    #[serde(rename = "MEDIUM")]
    Medium,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `notification.history.Notification`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryNotification {
    pub attachments: Option<Vec<HistoryAttachment>>,
    pub categories: Option<Vec<String>>,
    pub contacts: Option<Vec<HistoryContact>>,
    pub created_at: Option<String>,
    pub html: Option<String>,
    pub id: Option<String>,
    pub priority: Option<NotificationHistoryNotificationPriority>,
    pub summary: Option<String>,
    pub text: Option<String>,
    pub title: Option<String>,
}

/// `notification.reference.ReferenceCategory`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceReferenceCategory {
    pub description: Option<String>,
    pub name: Option<String>,
}

/// `notification.reference.ReferencePriority`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceReferencePriority {
    pub description: Option<String>,
    pub name: Option<NotificationHistoryNotificationPriority>,
}

/// `notification.reference.Reference`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceReference {
    pub categories: Option<Vec<ReferenceReferenceCategory>>,
    pub priorities: Option<Vec<ReferenceReferencePriority>>,
}

/// `notification.routing.RuleCondition`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingRuleCondition {
    pub category: Option<Vec<String>>,
    pub priority: Option<Vec<String>>,
}

/// `notification.routing.RuleContactMean`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingRuleContactMean {
    pub email: Option<String>,
    pub error: Option<String>,
    pub id: String,
    pub status: Option<NotificationContactMeanStatus>,
    #[serde(rename = "type")]
    pub kind: Option<NotificationContactMeanType>,
}

/// `notification.routing.Rule`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingRule {
    pub condition: RoutingRuleCondition,
    #[serde(default)]
    pub contact_means: Vec<RoutingRuleContactMean>,
    pub continue_: bool,
}

/// `notification.routing.PostInput`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingPostInput {
    pub active: bool,
    pub name: String,
    #[serde(default)]
    pub rules: Vec<RoutingRule>,
}

/// `notification.routing.Routing`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingRouting {
    pub active: Option<bool>,
    pub created_at: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub rules: Option<Vec<RoutingRule>>,
}

impl OvhClient {
    /// `GET /notification/contactMean` — Retrieve every contact mean
    pub async fn notification_contact_means(
        &self,
        page: &PageParams,
    ) -> Result<Vec<ContactMeanContactMean>> {
        self.get_page("/notification/contactMean", &[], page)
            .await
            .map(|p| p.items)
    }

    /// `POST /notification/contactMean` — Create a contact mean
    pub async fn notification_contact_mean_post(
        &self,
        body: &ContactMeanPostInput,
    ) -> Result<ContactMeanContactMean> {
        self.post_v2(
            "/notification/contactMean",
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /notification/contactMean/{contactMeanId}` — Delete the contact mean
    pub async fn notification_contact_mean_delete(&self, contact_mean_id: &str) -> Result<()> {
        self.delete(&format!(
            "/notification/contactMean/{}",
            percent_encode(contact_mean_id)
        ))
        .await
    }

    /// `GET /notification/contactMean/{contactMeanId}` — Retrieve information about a contact mean
    pub async fn notification_contact_mean(
        &self,
        contact_mean_id: &str,
    ) -> Result<ContactMeanContactMean> {
        self.get(&format!(
            "/notification/contactMean/{}",
            percent_encode(contact_mean_id)
        ))
        .await
    }

    /// `PUT /notification/contactMean/{contactMeanId}` — Update a contact mean
    pub async fn notification_contact_mean_put(
        &self,
        contact_mean_id: &str,
        body: &ContactMeanContactMean,
    ) -> Result<ContactMeanContactMean> {
        self.put_json(
            &format!(
                "/notification/contactMean/{}",
                percent_encode(contact_mean_id)
            ),
            body,
        )
        .await
    }

    /// `POST /notification/contactMean/{contactMeanId}/restartValidation` — Restart the validation process for this contact mean, if you did not receive the OTP
    pub async fn notification_contact_mean_restart_validation_post(
        &self,
        contact_mean_id: &str,
    ) -> Result<ContactMeanContactMean> {
        self.post_v2_no_body(
            &format!(
                "/notification/contactMean/{}/restartValidation",
                percent_encode(contact_mean_id)
            ),
            V2RequestOptions::default(),
        )
        .await
    }

    /// `GET /notification/contactMean/{contactMeanId}/task` — Get the list of tasks on a contact mean
    pub async fn notification_contact_mean_task(
        &self,
        contact_mean_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Task>> {
        self.get_page(
            &format!(
                "/notification/contactMean/{}/task",
                percent_encode(contact_mean_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /notification/contactMean/{contactMeanId}/task/{taskId}` — Get a task on a contact mean
    pub async fn notification_contact_mean_task_get(
        &self,
        contact_mean_id: &str,
        task_id: &str,
    ) -> Result<Task> {
        self.get(&format!(
            "/notification/contactMean/{}/task/{}",
            percent_encode(contact_mean_id),
            percent_encode(task_id)
        ))
        .await
    }

    /// `PUT /notification/contactMean/{contactMeanId}/task/{taskId}` — Update a task on a contact mean
    pub async fn notification_contact_mean_task_put(
        &self,
        contact_mean_id: &str,
        task_id: &str,
        body: &serde_json::Value,
    ) -> Result<Task> {
        self.put_json(
            &format!(
                "/notification/contactMean/{}/task/{}",
                percent_encode(contact_mean_id),
                percent_encode(task_id)
            ),
            body,
        )
        .await
    }

    /// `POST /notification/contactMean/{contactMeanId}/validate` — Validate this contact mean
    pub async fn notification_contact_mean_validate_post(
        &self,
        contact_mean_id: &str,
        body: &ContactMeanValidateInput,
    ) -> Result<ContactMeanContactMean> {
        self.post_v2(
            &format!(
                "/notification/contactMean/{}/validate",
                percent_encode(contact_mean_id)
            ),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `GET /notification/history` — Retrieve every notification sent to you
    pub async fn notification_historys(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<HistoryNotification>> {
        self.get_page(
            &Self::append_query("/notification/history", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /notification/history/{notificationId}` — Retrieve information about a notification sent to you
    pub async fn notification_history(&self, notification_id: &str) -> Result<HistoryNotification> {
        self.get(&format!(
            "/notification/history/{}",
            percent_encode(notification_id)
        ))
        .await
    }

    /// `GET /notification/history/{notificationId}/attachment/{attachmentName}` — Get a notification attachment
    pub async fn notification_history_attachment(
        &self,
        attachment_name: &str,
        notification_id: &str,
    ) -> Result<HistoryAttachment> {
        self.get(&format!(
            "/notification/history/{}/attachment/{}",
            percent_encode(notification_id),
            percent_encode(attachment_name)
        ))
        .await
    }

    /// `GET /notification/reference` — Retrieve data referential for /notification endpoints
    pub async fn notification_references(&self) -> Result<ReferenceReference> {
        self.get("/notification/reference").await
    }

    /// `GET /notification/routing` — Retrieve every routing
    pub async fn notification_routings(&self, page: &PageParams) -> Result<Vec<RoutingRouting>> {
        self.get_page("/notification/routing", &[], page)
            .await
            .map(|p| p.items)
    }

    /// `POST /notification/routing` — Create a routing
    pub async fn notification_routing_post(
        &self,
        body: &RoutingPostInput,
    ) -> Result<RoutingRouting> {
        self.post_v2("/notification/routing", body, V2RequestOptions::default())
            .await
    }

    /// `DELETE /notification/routing/{routingId}` — Delete the routing
    pub async fn notification_routing_delete(&self, routing_id: &str) -> Result<()> {
        self.delete(&format!(
            "/notification/routing/{}",
            percent_encode(routing_id)
        ))
        .await
    }

    /// `GET /notification/routing/{routingId}` — Retrieve information about a routing
    pub async fn notification_routing(&self, routing_id: &str) -> Result<RoutingRouting> {
        self.get(&format!(
            "/notification/routing/{}",
            percent_encode(routing_id)
        ))
        .await
    }

    /// `PUT /notification/routing/{routingId}` — Update a routing
    pub async fn notification_routing_put(
        &self,
        routing_id: &str,
        body: &RoutingRouting,
    ) -> Result<RoutingRouting> {
        self.put_json(
            &format!("/notification/routing/{}", percent_encode(routing_id)),
            body,
        )
        .await
    }
}
