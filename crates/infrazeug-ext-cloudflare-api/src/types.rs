//! Shared Cloudflare API v4 envelope types.

use serde::{Deserialize, Serialize};

/// Pagination metadata on collection responses.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ResultInfo {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub count: Option<u32>,
    pub total_count: Option<u32>,
}

impl ResultInfo {
    /// Whether another page is likely available.
    pub fn has_more(&self) -> bool {
        match (self.page, self.per_page, self.total_count) {
            (Some(page), Some(per_page), Some(total)) => page * per_page < total,
            _ => false,
        }
    }
}

/// One entry in the API `errors` array.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiErrorEntry {
    pub code: Option<u64>,
    pub message: Option<String>,
}

/// The standard Cloudflare v4 response envelope.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudflareResponse<T> {
    pub success: bool,
    #[serde(default)]
    pub errors: Vec<ApiErrorEntry>,
    #[serde(default)]
    pub messages: Vec<ApiErrorEntry>,
    pub result: Option<T>,
    #[serde(default)]
    pub result_info: Option<ResultInfo>,
}

/// Query parameters for list endpoints (`page`, `per_page`, …).
#[derive(Debug, Clone, Default)]
pub struct ListQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub name: Option<String>,
    pub status: Option<String>,
    pub record_type: Option<String>,
    pub content: Option<String>,
    pub proxied: Option<bool>,
}

impl ListQuery {
    pub fn as_params(&self) -> Vec<(&str, String)> {
        let mut out = Vec::new();
        if let Some(page) = self.page {
            out.push(("page", page.to_string()));
        }
        if let Some(per_page) = self.per_page {
            out.push(("per_page", per_page.to_string()));
        }
        if let Some(name) = &self.name {
            out.push(("name", name.clone()));
        }
        if let Some(status) = &self.status {
            out.push(("status", status.clone()));
        }
        if let Some(record_type) = &self.record_type {
            out.push(("type", record_type.clone()));
        }
        if let Some(content) = &self.content {
            out.push(("content", content.clone()));
        }
        if let Some(proxied) = self.proxied {
            out.push(("proxied", proxied.to_string()));
        }
        out
    }
}
