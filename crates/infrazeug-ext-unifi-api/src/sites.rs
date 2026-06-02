//! Site listing (`/api/self/sites`).

use serde::{Deserialize, Serialize};

/// A UniFi site the authenticated account can administer.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Site {
    /// Internal object id.
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Site shortname used in API paths (e.g. `default`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Human-readable site description.
    #[serde(rename = "desc", default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
