//! Zone settings (`/zones/{zone_id}/settings/{setting_id}`).

use crate::client::CloudflareClient;
use crate::error::Result;
use crate::types::ListQuery;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A single zone setting returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ZoneSetting {
    pub id: String,
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_on: Option<String>,
}

/// Body for `PATCH /zones/{zone_id}/settings/{setting_id}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZoneSettingUpdate {
    pub value: Value,
}

impl ZoneSettingUpdate {
    pub fn string(value: impl Into<String>) -> Self {
        Self {
            value: Value::String(value.into()),
        }
    }
}

/// Well-known zone setting ids.
pub mod setting_id {
    pub const SSL: &str = "ssl";
    pub const ALWAYS_USE_HTTPS: &str = "always_use_https";
    pub const AUTOMATIC_HTTPS_REWRITES: &str = "automatic_https_rewrites";
    pub const MIN_TLS_VERSION: &str = "min_tls_version";
}

impl CloudflareClient {
    /// `GET /zones/{zone_id}/settings` — list all zone settings.
    pub async fn zone_settings(&self, zone_id: &str) -> Result<Vec<ZoneSetting>> {
        let path = format!("/zones/{}/settings", self.encode_path(zone_id));
        let (settings, _) = self.get(&path, &ListQuery::default()).await?;
        Ok(settings)
    }

    /// `GET /zones/{zone_id}/settings/{setting_id}` — fetch one setting.
    pub async fn zone_setting(&self, zone_id: &str, setting_id: &str) -> Result<ZoneSetting> {
        let path = format!(
            "/zones/{}/settings/{}",
            self.encode_path(zone_id),
            self.encode_path(setting_id)
        );
        let (setting, _) = self.get(&path, &ListQuery::default()).await?;
        Ok(setting)
    }

    /// `PATCH /zones/{zone_id}/settings/{setting_id}` — update one setting.
    pub async fn update_zone_setting(
        &self,
        zone_id: &str,
        setting_id: &str,
        body: &ZoneSettingUpdate,
    ) -> Result<ZoneSetting> {
        let path = format!(
            "/zones/{}/settings/{}",
            self.encode_path(zone_id),
            self.encode_path(setting_id)
        );
        self.patch_json(&path, body).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::decode_body;
    use reqwest::StatusCode;

    #[test]
    fn decode_ssl_setting() {
        let body = r#"{
            "success": true,
            "errors": [],
            "messages": [],
            "result": {
                "id": "ssl",
                "value": "full",
                "editable": true,
                "modified_on": "2014-01-01T05:20:00.12345Z"
            }
        }"#;
        let (setting, _): (ZoneSetting, _) = decode_body(StatusCode::OK, body).unwrap();
        assert_eq!(setting.id, "ssl");
        assert_eq!(setting.value, Value::String("full".into()));
    }

    #[test]
    fn decode_always_use_https_setting() {
        let body = r#"{
            "success": true,
            "errors": [],
            "messages": [],
            "result": {
                "id": "always_use_https",
                "value": "on",
                "editable": true
            }
        }"#;
        let (setting, _): (ZoneSetting, _) = decode_body(StatusCode::OK, body).unwrap();
        assert_eq!(setting.id, "always_use_https");
        assert_eq!(setting.value, Value::String("on".into()));
    }

    #[test]
    fn update_serializes_value() {
        let body = ZoneSettingUpdate::string("strict");
        let json = serde_json::to_value(body).unwrap();
        assert_eq!(json["value"], "strict");
    }
}
