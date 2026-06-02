//! IAM resource metadata shared across product surfaces (`iam.ResourceMetadata`).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// State of an IAM resource (`iam.ResourceMetadata.StateEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceState {
    Expired,
    InCreation,
    Ok,
    Suspended,
    /// A value not present in the schema this crate was built against.
    #[serde(other)]
    Unknown,
}

/// IAM metadata embedded in service models (`iam.ResourceMetadata`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMetadata {
    /// Human-readable display name, if any.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Resource UUID.
    pub id: String,
    /// Current IAM state, if exposed.
    #[serde(default)]
    pub state: Option<ResourceState>,
    /// Arbitrary resource tags.
    #[serde(default)]
    pub tags: Option<HashMap<String, String>>,
    /// Uniform Resource Name.
    pub urn: String,
}
