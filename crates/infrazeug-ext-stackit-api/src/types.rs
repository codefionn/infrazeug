//! Shared STACKIT IaaS API types.

use serde::{Deserialize, Serialize};

/// A reference to another IaaS resource (image, volume, …).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSource {
    pub id: String,
    #[serde(rename = "type")]
    pub source_type: String,
}

/// Paginated list wrapper returned by collection endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ItemList<T> {
    #[serde(default)]
    pub items: Vec<T>,
}
