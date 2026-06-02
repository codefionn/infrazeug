//! Controller system information (`/stat/sysinfo`).

use crate::client::UnifiClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

const STAT: &str = "sysinfo";

/// Controller / console system information. Unmodelled fields round-trip through
/// [`extra`](Self::extra).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SysInfo {
    /// Network application version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Controller uptime in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<i64>,
    /// Fields this client does not model (timezone, console details, …).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl UnifiClient {
    /// `GET /stat/sysinfo` — controller system information (the single record).
    pub async fn sysinfo(&self) -> Result<Option<SysInfo>> {
        Ok(self.stat_list::<SysInfo>(STAT).await?.into_iter().next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UnifiResponse;

    #[test]
    fn deserializes_sysinfo_envelope() {
        let body = r#"{"meta":{"rc":"ok"},"data":[{
            "version":"8.4.62","build":"atag_8.4.62_12345","hostname":"udm-pro",
            "name":"Home","uptime":123456,"timezone":"Europe/Berlin"
        }]}"#;
        let env: UnifiResponse<SysInfo> = serde_json::from_str(body).unwrap();
        let info = &env.data[0];
        assert_eq!(info.version.as_deref(), Some("8.4.62"));
        assert_eq!(info.uptime, Some(123456));
        assert!(info.extra.contains_key("timezone"));
    }
}
