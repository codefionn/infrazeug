//! User groups — per-client bandwidth limit profiles (`/rest/usergroup`).

use crate::client::{first_item, UnifiClient};
use crate::error::Result;
use serde::{Deserialize, Serialize};

const RESOURCE: &str = "usergroup";

/// A user group defining up/down rate limits applied to its member clients.
///
/// Rates are in kbps; `-1` means unlimited. Unmodelled fields round-trip through
/// [`extra`](Self::extra).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct UserGroup {
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    /// Download rate limit (kbps); `-1` = unlimited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qos_rate_max_down: Option<i32>,
    /// Upload rate limit (kbps); `-1` = unlimited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qos_rate_max_up: Option<i32>,
    /// Fields this client does not model, preserved verbatim across updates.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl UnifiClient {
    /// `GET /rest/usergroup` — list user groups on the site.
    pub async fn user_groups(&self) -> Result<Vec<UserGroup>> {
        self.rest_list(RESOURCE).await
    }

    /// `POST /rest/usergroup` — create a user group.
    pub async fn create_user_group(&self, body: &UserGroup) -> Result<UserGroup> {
        first_item(self.rest_create(RESOURCE, body).await?, "user group")
    }

    /// `PUT /rest/usergroup/{id}` — replace a user group.
    pub async fn update_user_group(&self, id: &str, body: &UserGroup) -> Result<UserGroup> {
        first_item(self.rest_update(RESOURCE, id, body).await?, "user group")
    }

    /// `DELETE /rest/usergroup/{id}` — delete a user group.
    pub async fn delete_user_group(&self, id: &str) -> Result<()> {
        self.rest_delete(RESOURCE, id).await
    }
}
