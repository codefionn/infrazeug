//! Active clients / stations (`/stat/sta`) and client management (`/cmd/stamgr`).

use crate::client::UnifiClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

const STAT: &str = "sta";
const MGR: &str = "stamgr";

/// A currently-connected client (station). Only commonly used fields are typed;
/// the rest (signal, rates, per-radio stats, …) round-trip through
/// [`extra`](Self::extra).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ActiveClient {
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub mac: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    /// User-assigned name (alias), when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_wired: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_guest: Option<bool>,
    /// SSID, for wireless clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub essid: Option<String>,
    /// MAC of the AP a wireless client is associated with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ap_mac: Option<String>,
    /// MAC of the switch a wired client is connected to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sw_mac: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rx_bytes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_bytes: Option<i64>,
    /// Fields this client does not model.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl UnifiClient {
    /// `GET /stat/sta` — list currently-connected clients.
    pub async fn active_clients(&self) -> Result<Vec<ActiveClient>> {
        self.stat_list(STAT).await
    }

    /// `POST /cmd/stamgr {cmd: block-sta}` — block a client.
    pub async fn block_client(&self, mac: &str) -> Result<()> {
        self.cmd(MGR, &serde_json::json!({ "cmd": "block-sta", "mac": mac }))
            .await
    }

    /// `POST /cmd/stamgr {cmd: unblock-sta}` — unblock a client.
    pub async fn unblock_client(&self, mac: &str) -> Result<()> {
        self.cmd(
            MGR,
            &serde_json::json!({ "cmd": "unblock-sta", "mac": mac }),
        )
        .await
    }

    /// `POST /cmd/stamgr {cmd: kick-sta}` — force a wireless client to reconnect.
    pub async fn reconnect_client(&self, mac: &str) -> Result<()> {
        self.cmd(MGR, &serde_json::json!({ "cmd": "kick-sta", "mac": mac }))
            .await
    }

    /// `POST /cmd/stamgr {cmd: forget-sta}` — forget clients (remove their history).
    pub async fn forget_clients(&self, macs: &[&str]) -> Result<()> {
        self.cmd(
            MGR,
            &serde_json::json!({ "cmd": "forget-sta", "macs": macs }),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UnifiResponse;

    #[test]
    fn deserializes_active_client_envelope() {
        let body = r#"{"meta":{"rc":"ok"},"data":[{
            "mac":"11:22:33:44:55:66","hostname":"laptop","ip":"10.0.0.42",
            "is_wired":false,"is_guest":false,"essid":"home","ap_mac":"aa:bb:cc:dd:ee:ff",
            "uptime":3600,"rx_bytes":1000,"tx_bytes":2000,"signal":-55
        }]}"#;
        let env: UnifiResponse<ActiveClient> = serde_json::from_str(body).unwrap();
        let c = &env.data[0];
        assert_eq!(c.mac, "11:22:33:44:55:66");
        assert_eq!(c.essid.as_deref(), Some("home"));
        assert_eq!(c.is_wired, Some(false));
        assert!(c.extra.contains_key("signal"));
    }
}
