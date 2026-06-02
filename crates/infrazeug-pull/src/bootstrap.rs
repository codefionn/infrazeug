use crate::error::{PullError, Result};
use crate::fetch_auth::FetchAuth;
use crate::mode::PullMode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bootstrap {
    pub machine_id: Uuid,
    pub plan_url: String,
    pub agent_url: String,
    pub agent_digest: String,
    pub agent_signer: String,
    pub plan_signer: String,
    pub machine_key: PathBuf,
    #[serde(default)]
    pub fetch_auth: FetchAuth,
    #[serde(default)]
    pub poll_interval: Option<Duration>,
}

impl Bootstrap {
    pub fn pull_mode(&self) -> PullMode {
        PullMode::from_poll_interval(self.poll_interval)
    }
}

pub fn parse_bootstrap(bytes: &[u8]) -> Result<Bootstrap> {
    if let Ok(s) = std::str::from_utf8(bytes) {
        if let Ok(b) = toml::from_str::<Bootstrap>(s) {
            return Ok(b);
        }
    }
    if let Ok(b) = serde_json::from_slice::<Bootstrap>(bytes) {
        return Ok(b);
    }
    if let Ok(b) = parse_cloud_config(bytes) {
        return Ok(b);
    }
    parse_ignition(bytes)
}

/// `#cloud-config` YAML: extract first `content: |` block that looks like bootstrap TOML.
fn parse_cloud_config(bytes: &[u8]) -> Result<Bootstrap> {
    let s = std::str::from_utf8(bytes).map_err(|e| PullError::Bootstrap(e.to_string()))?;
    if !s.contains("#cloud-config") && !s.contains("machine_id") {
        return Err(PullError::Bootstrap("not cloud-config".into()));
    }
    let mut in_block = false;
    let mut block = String::new();
    for line in s.lines() {
        if line.trim_start().starts_with("content:") && line.contains('|') {
            in_block = true;
            continue;
        }
        if in_block {
            if !line.starts_with(' ') && !line.starts_with('\t') && !line.is_empty() {
                break;
            }
            block.push_str(line.trim_start());
            block.push('\n');
        }
    }
    if block.is_empty() {
        return toml::from_str(s).map_err(|e| PullError::Bootstrap(e.to_string()));
    }
    toml::from_str(&block).map_err(|e| PullError::Bootstrap(e.to_string()))
}

/// Ignition JSON: `storage.files[].contents.source` base64 or inline bootstrap object.
fn parse_ignition(bytes: &[u8]) -> Result<Bootstrap> {
    let doc: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| PullError::Bootstrap(e.to_string()))?;
    if doc.get("machine_id").is_some() {
        return serde_json::from_value(doc).map_err(|e| PullError::Bootstrap(e.to_string()));
    }
    let files = doc
        .pointer("/storage/files")
        .and_then(|v| v.as_array())
        .ok_or_else(|| PullError::Bootstrap("ignition missing storage.files".into()))?;
    for f in files {
        if let Some(obj) = f.get("contents").and_then(|c| c.as_object()) {
            if let Some(src) = obj.get("source").and_then(|s| s.as_str()) {
                if let Some(payload) = src.strip_prefix("data:,") {
                    return parse_bootstrap(payload.as_bytes());
                }
            }
        }
        if let Some(inline) = f.get("inline") {
            return serde_json::from_value(inline.clone())
                .map_err(|e| PullError::Bootstrap(e.to_string()));
        }
    }
    Err(PullError::Bootstrap("no bootstrap in ignition".into()))
}
