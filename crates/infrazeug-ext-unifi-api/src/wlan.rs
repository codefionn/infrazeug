//! Wireless network configuration (`/rest/wlanconf`).

use crate::client::{first_item, UnifiClient};
use crate::error::Result;
use serde::{Deserialize, Serialize};

const RESOURCE: &str = "wlanconf";

/// A wireless network (SSID) as stored by the controller.
///
/// Only the commonly managed fields are typed; everything else round-trips through
/// [`extra`](Self::extra) so a read-modify-write update preserves fields this client
/// does not model.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct WlanConf {
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// `open`, `wpapsk`, `wpaeap`, …
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<String>,
    /// Pre-shared key (for `wpapsk`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_passphrase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wpa_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wpa_enc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_guest: Option<bool>,
    /// Network this SSID is bridged onto (the modern VLAN binding).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub networkconf_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usergroup_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wlangroup_id: Option<String>,
    /// AP groups this SSID is broadcast on (UniFi OS).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ap_group_ids: Option<Vec<String>>,
    /// Fields this client does not model, preserved verbatim across updates.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl UnifiClient {
    /// `GET /rest/wlanconf` — list wireless networks on the site.
    pub async fn wlans(&self) -> Result<Vec<WlanConf>> {
        self.rest_list(RESOURCE).await
    }

    /// `POST /rest/wlanconf` — create a wireless network.
    pub async fn create_wlan(&self, body: &WlanConf) -> Result<WlanConf> {
        first_item(self.rest_create(RESOURCE, body).await?, "wlan")
    }

    /// `PUT /rest/wlanconf/{id}` — replace a wireless network.
    pub async fn update_wlan(&self, id: &str, body: &WlanConf) -> Result<WlanConf> {
        first_item(self.rest_update(RESOURCE, id, body).await?, "wlan")
    }

    /// `DELETE /rest/wlanconf/{id}` — delete a wireless network.
    pub async fn delete_wlan(&self, id: &str) -> Result<()> {
        self.rest_delete(RESOURCE, id).await
    }
}
