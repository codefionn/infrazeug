//! Async API request tracking (`/requests`).

use crate::client::IonosClient;
use crate::error::{IonosError, Result};
use crate::types::{Collection, ListQuery, ResourceReference};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Request resource returned by the Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RequestMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<RequestProperties>,
}

/// Request metadata including embedded status.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RequestMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_status: Option<RequestStatus>,
}

/// Request status resource.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RequestStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RequestStatusMetadata>,
}

/// Status metadata for a request.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RequestStatusMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<RequestTargetStatus>>,
}

/// Per-target status within a request.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RequestTargetStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<ResourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Request properties (original HTTP call details).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RequestProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// Filter for listing requests by status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RequestStatusFilter {
    Queued,
    Running,
    Done,
    Failed,
}

/// Options for [`IonosClient::wait_request`].
#[derive(Debug, Clone)]
pub struct WaitRequestOptions {
    /// Poll interval (default 1s).
    pub poll_interval: Duration,
    /// Maximum wait time (default 5 minutes).
    pub timeout: Duration,
}

impl Default for WaitRequestOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(1),
            timeout: Duration::from_secs(300),
        }
    }
}

impl IonosClient {
    /// `GET /requests` — list API requests made by the current user.
    pub async fn requests(
        &self,
        query: &ListQuery,
        status: Option<RequestStatusFilter>,
    ) -> Result<Collection<Request>> {
        let mut params = query.as_params();
        if let Some(status) = status {
            let value = serde_json::to_value(status)?;
            if let Some(s) = value.as_str() {
                params.push(("status", s.to_string()));
            }
        }
        let resp = self
            .send_cloud(reqwest::Method::GET, "/requests", &params, None, None)
            .await?;
        let (status, body) = super::client::consume(resp).await?;
        super::client::decode(status, &body)
    }

    /// `GET /requests/{id}` — retrieve one request.
    pub async fn request(&self, request_id: &str, query: &ListQuery) -> Result<Request> {
        self.get(
            &format!("/requests/{}", self.encode_path(request_id)),
            query,
        )
        .await
    }

    /// `GET /requests/{id}/status` — retrieve request status.
    pub async fn request_status(
        &self,
        request_id: &str,
        query: &ListQuery,
    ) -> Result<RequestStatus> {
        self.get(
            &format!("/requests/{}/status", self.encode_path(request_id)),
            query,
        )
        .await
    }

    /// Poll a request until it reaches `DONE` or `FAILED`.
    pub async fn wait_request(
        &self,
        request_id: &str,
        options: &WaitRequestOptions,
    ) -> Result<RequestStatus> {
        let deadline = tokio::time::Instant::now() + options.timeout;
        loop {
            let status = self
                .request_status(request_id, &ListQuery::default())
                .await?;
            let state = status
                .metadata
                .as_ref()
                .and_then(|m| m.status.as_deref())
                .unwrap_or("");
            match state {
                "DONE" => return Ok(status),
                "FAILED" => {
                    let message = status
                        .metadata
                        .as_ref()
                        .and_then(|m| m.message.clone())
                        .unwrap_or_else(|| "request failed".into());
                    return Err(IonosError::Api {
                        status: 0,
                        codes: vec![],
                        message,
                    });
                }
                _ if tokio::time::Instant::now() >= deadline => {
                    return Err(IonosError::Api {
                        status: 0,
                        codes: vec![],
                        message: format!("request {request_id} timed out in state {state}"),
                    });
                }
                _ => tokio::time::sleep(options.poll_interval).await,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_filter_serializes() {
        let json = serde_json::to_string(&RequestStatusFilter::Running).unwrap();
        assert_eq!(json, "\"RUNNING\"");
    }
}
