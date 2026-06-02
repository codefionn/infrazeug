//! Shared UniFi controller API envelope types.
//!
//! Every controller REST endpoint wraps its payload in `{ "meta": { "rc": "ok" },
//! "data": [ … ] }`. A logical failure is reported either with a non-2xx status or
//! with `meta.rc == "error"` and a `meta.msg` UniFi error code.

use serde::Deserialize;

/// Result metadata returned alongside every payload.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Meta {
    /// Result code: `"ok"` on success, `"error"` otherwise.
    #[serde(default)]
    pub rc: String,
    /// UniFi error code (e.g. `api.err.NoSiteContext`) when `rc != "ok"`.
    #[serde(default)]
    pub msg: Option<String>,
}

impl Meta {
    /// Whether the controller reported success.
    pub fn is_ok(&self) -> bool {
        self.rc == "ok"
    }
}

/// The standard UniFi controller response envelope.
#[derive(Debug, Clone, Deserialize)]
pub struct UnifiResponse<T> {
    #[serde(default)]
    pub meta: Meta,
    #[serde(default = "Vec::new")]
    pub data: Vec<T>,
}
