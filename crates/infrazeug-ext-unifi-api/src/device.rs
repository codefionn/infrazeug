//! UniFi devices — APs, switches, gateways.
//!
//! Read live state (`/stat/device`), push settings (`/rest/device`), and run
//! management commands (`/cmd/devmgr`: restart, adopt, force-provision).

use crate::client::{first_item, UnifiClient};
use crate::error::Result;
use serde::{Deserialize, Serialize};

const STAT: &str = "device";
const REST: &str = "device";
const MGR: &str = "devmgr";

/// A UniFi device and its current state. Only commonly used fields are typed;
/// everything else (radios, port table, stats, …) round-trips through
/// [`extra`](Self::extra).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Device {
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub mac: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Device class: `uap` (access point), `usw` (switch), `ugw` / `udm` (gateway).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub device_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adopted: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    /// Connection state (`1` = connected; `0` = disconnected; other values are
    /// transitional such as provisioning/adopting).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    /// Seconds since the device last booted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<i64>,
    /// Number of clients currently associated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_sta: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upgradable: Option<bool>,
    /// Fields this client does not model, preserved verbatim across updates.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Device {
    /// `true` when [`state`](Self::state) reports the device as connected.
    pub fn is_connected(&self) -> bool {
        self.state == Some(1)
    }
}

impl UnifiClient {
    /// `GET /stat/device` — list all devices and their live state.
    pub async fn devices(&self) -> Result<Vec<Device>> {
        self.stat_list(STAT).await
    }

    /// `GET /stat/device/{mac}` — one device by MAC, or `None` if not present.
    pub async fn device(&self, mac: &str) -> Result<Option<Device>> {
        let suffix = format!("{STAT}/{}", self.encode_path(mac));
        Ok(self.stat_list::<Device>(&suffix).await?.into_iter().next())
    }

    /// `PUT /rest/device/{id}` — push device settings (e.g. name, `disabled`).
    pub async fn update_device(&self, id: &str, body: &Device) -> Result<Device> {
        first_item(self.rest_update(REST, id, body).await?, "device")
    }

    /// `POST /cmd/devmgr {cmd: restart}` — reboot a device.
    pub async fn restart_device(&self, mac: &str) -> Result<()> {
        self.cmd(MGR, &serde_json::json!({ "cmd": "restart", "mac": mac }))
            .await
    }

    /// `POST /cmd/devmgr {cmd: adopt}` — adopt a pending device.
    pub async fn adopt_device(&self, mac: &str) -> Result<()> {
        self.cmd(MGR, &serde_json::json!({ "cmd": "adopt", "mac": mac }))
            .await
    }

    /// `POST /cmd/devmgr {cmd: force-provision}` — re-push configuration to a device.
    pub async fn force_provision(&self, mac: &str) -> Result<()> {
        self.cmd(
            MGR,
            &serde_json::json!({ "cmd": "force-provision", "mac": mac }),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UnifiResponse;

    #[test]
    fn deserializes_device_envelope() {
        let body = r#"{"meta":{"rc":"ok"},"data":[{
            "_id":"a1","mac":"aa:bb:cc:dd:ee:ff","name":"AP-Office","type":"uap",
            "model":"U6LR","version":"6.6.55","adopted":true,"state":1,
            "ip":"10.0.0.2","uptime":86400,"num_sta":7,"radio_table":[{"name":"ng"}]
        }]}"#;
        let env: UnifiResponse<Device> = serde_json::from_str(body).unwrap();
        assert!(env.meta.is_ok());
        let d = &env.data[0];
        assert_eq!(d.mac, "aa:bb:cc:dd:ee:ff");
        assert_eq!(d.device_type.as_deref(), Some("uap"));
        assert_eq!(d.num_sta, Some(7));
        assert!(d.is_connected());
        // Unmodelled fields survive for a non-destructive read-modify-write.
        assert!(d.extra.contains_key("radio_table"));
    }
}
