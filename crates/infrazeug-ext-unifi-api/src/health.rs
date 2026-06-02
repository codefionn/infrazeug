//! Site health by subsystem (`/stat/health`).

use crate::client::UnifiClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

const STAT: &str = "health";

/// Health of one controller subsystem (`wlan`, `wan`, `lan`, `vpn`, `www`).
///
/// Per-subsystem counters vary; unmodelled fields round-trip through
/// [`extra`](Self::extra).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SubsystemHealth {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subsystem: Option<String>,
    /// `ok`, `warning`, `error`, or `unknown`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_user: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_guest: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_iot: Option<i64>,
    /// Access points (in the `wlan` subsystem).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_ap: Option<i64>,
    /// Switches (in the `lan` subsystem).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_sw: Option<i64>,
    /// Gateways (in the `wan` subsystem).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_gw: Option<i64>,
    /// Fields this client does not model.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl SubsystemHealth {
    /// `true` when this subsystem reports `status == "ok"`.
    pub fn is_ok(&self) -> bool {
        self.status.as_deref() == Some("ok")
    }
}

impl UnifiClient {
    /// `GET /stat/health` — per-subsystem health for the site.
    pub async fn health(&self) -> Result<Vec<SubsystemHealth>> {
        self.stat_list(STAT).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UnifiResponse;

    #[test]
    fn deserializes_health_envelope() {
        let body = r#"{"meta":{"rc":"ok"},"data":[
            {"subsystem":"wlan","status":"ok","num_user":10,"num_guest":2,"num_ap":3},
            {"subsystem":"www","status":"warning","latency":12}
        ]}"#;
        let env: UnifiResponse<SubsystemHealth> = serde_json::from_str(body).unwrap();
        assert_eq!(env.data.len(), 2);
        assert!(env.data[0].is_ok());
        assert_eq!(env.data[0].num_ap, Some(3));
        assert!(!env.data[1].is_ok());
        assert!(env.data[1].extra.contains_key("latency"));
    }
}
