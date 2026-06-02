//! Cube server template discovery (`/templates`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};

/// Template resource (read-only).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Template {
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
    pub properties: Option<TemplateProperties>,
}

/// Template properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cores: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

impl IonosClient {
    /// `GET /templates` — list available Cube server templates.
    pub async fn templates(&self, query: &ListQuery) -> Result<Collection<Template>> {
        self.get("/templates", query).await
    }

    /// `GET /templates/{id}` — retrieve one template.
    pub async fn template(&self, template_id: &str, query: &ListQuery) -> Result<Template> {
        self.get(
            &format!("/templates/{}", self.encode_path(template_id)),
            query,
        )
        .await
    }
}
