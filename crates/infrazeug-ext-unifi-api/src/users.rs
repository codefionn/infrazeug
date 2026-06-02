//! Known clients (`/rest/user`) — naming, fixed-IP reservations, and group binding.

use crate::client::{first_item, UnifiClient};
use crate::error::Result;
use serde::{Deserialize, Serialize};

const RESOURCE: &str = "user";

/// A known client device, keyed by MAC address. This is how DHCP reservations
/// (fixed IPs), friendly names, and bandwidth-group membership are managed.
///
/// Unmodelled fields round-trip through [`extra`](Self::extra).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct User {
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Client MAC address (the natural key).
    pub mac: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Reserved IP address (when [`use_fixedip`](Self::use_fixedip) is set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_fixedip: Option<bool>,
    /// Network the reservation belongs to (required for a fixed IP).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_id: Option<String>,
    /// User group (bandwidth profile) the client is assigned to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usergroup_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Fields this client does not model, preserved verbatim across updates.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl UnifiClient {
    /// `GET /rest/user` — list known clients on the site.
    pub async fn users(&self) -> Result<Vec<User>> {
        self.rest_list(RESOURCE).await
    }

    /// `POST /rest/user` — register a known client.
    pub async fn create_user(&self, body: &User) -> Result<User> {
        first_item(self.rest_create(RESOURCE, body).await?, "user")
    }

    /// `PUT /rest/user/{id}` — replace a known client.
    pub async fn update_user(&self, id: &str, body: &User) -> Result<User> {
        first_item(self.rest_update(RESOURCE, id, body).await?, "user")
    }

    /// `DELETE /rest/user/{id}` — forget a known client.
    pub async fn delete_user(&self, id: &str) -> Result<()> {
        self.rest_delete(RESOURCE, id).await
    }
}
