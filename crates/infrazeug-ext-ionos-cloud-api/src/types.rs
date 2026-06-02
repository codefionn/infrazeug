//! Shared IONOS Cloud API v6 resource types.

use serde::{Deserialize, Serialize};

/// Common query parameters for list and get operations.
#[derive(Debug, Clone, Default)]
pub struct ListQuery {
    /// Detail depth (0–10). Higher values inline more child resources.
    pub depth: Option<u8>,
    /// Pagination offset.
    pub offset: Option<u32>,
    /// Page size (default 100).
    pub limit: Option<u32>,
}

impl ListQuery {
    /// Query preset for `/um/*` endpoints (max depth 1 per API docs).
    pub fn um() -> Self {
        Self {
            depth: Some(1),
            ..Default::default()
        }
    }

    /// Convert to `(key, value)` pairs for the query string.
    pub fn as_params(&self) -> Vec<(&str, String)> {
        let mut out = Vec::new();
        if let Some(depth) = self.depth {
            out.push(("depth", depth.to_string()));
        }
        if let Some(offset) = self.offset {
            out.push(("offset", offset.to_string()));
        }
        if let Some(limit) = self.limit {
            out.push(("limit", limit.to_string()));
        }
        out
    }
}

/// Metadata attached to most IONOS resources.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ElementMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_by_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

/// Lightweight reference to another API resource.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

/// Pagination links on collection responses.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CollectionLinks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<String>,
    #[serde(rename = "self", skip_serializing_if = "Option::is_none")]
    pub self_href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

/// A paginated collection of resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub collection_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(rename = "_links", skip_serializing_if = "Option::is_none")]
    pub links: Option<CollectionLinks>,
}

/// `GET /` — API version information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_query_params() {
        let q = ListQuery {
            depth: Some(1),
            offset: Some(0),
            limit: Some(50),
        };
        let params = q.as_params();
        assert_eq!(params.len(), 3);
        assert!(params.contains(&("depth", "1".into())));
    }

    #[test]
    fn deserialize_api_info() {
        let json = r#"{"name":"CLOUD API","version":"6.0"}"#;
        let info: ApiInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.version.as_deref(), Some("6.0"));
    }
}
