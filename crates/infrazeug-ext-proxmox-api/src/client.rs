//! HTTP client for the Proxmox VE API.

use crate::auth::Auth;
use crate::error::{ProxmoxError, Result};
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Default Proxmox API port and path prefix.
pub(crate) const API_PREFIX: &str = "/api2/json";

/// Connection configuration for a Proxmox VE API client.
#[derive(Debug, Clone)]
pub struct ProxmoxConfig {
    /// Base host URL, e.g. `https://pve.example.com:8006` (no trailing `/api2/json`).
    pub host: String,
    /// Accept self-signed / invalid TLS certificates (common for Proxmox hosts).
    pub insecure_tls: bool,
    /// Authentication method (API token or login ticket).
    pub auth: Auth,
}

impl ProxmoxConfig {
    /// Create a configuration for `host` (e.g. `https://pve.example.com:8006`).
    pub fn new(host: impl Into<String>, auth: Auth) -> Self {
        Self {
            host: host.into().trim_end_matches('/').to_string(),
            insecure_tls: false,
            auth,
        }
    }

    /// Accept invalid/self-signed TLS certificates.
    pub fn insecure_tls(mut self, insecure: bool) -> Self {
        self.insecure_tls = insecure;
        self
    }

    /// API root, e.g. `https://pve.example.com:8006/api2/json`.
    pub(crate) fn api_base(&self) -> String {
        format!("{}{}", self.host.trim_end_matches('/'), API_PREFIX)
    }
}

/// An authenticated Proxmox VE API client.
#[derive(Clone)]
pub struct ProxmoxClient {
    http: Client,
    config: ProxmoxConfig,
}

impl ProxmoxClient {
    /// Build a client with the given configuration.
    ///
    /// When TLS-client construction fails (extremely rare), this falls back to a
    /// default secure client; use [`ProxmoxClient::try_new`] to surface that error.
    pub fn new(config: ProxmoxConfig) -> Self {
        Self::try_new(config.clone()).unwrap_or(Self {
            http: Client::new(),
            config,
        })
    }

    /// Build a client, surfacing a TLS setup failure instead of falling back.
    pub fn try_new(config: ProxmoxConfig) -> Result<Self> {
        let builder = Client::builder().danger_accept_invalid_certs(config.insecure_tls);
        match builder.build() {
            Ok(http) => Ok(Self { http, config }),
            Err(e) => Err(ProxmoxError::Build(e.to_string())),
        }
    }

    /// Use a pre-configured [`reqwest::Client`] (custom timeouts, proxy, …).
    pub fn with_http_client(mut self, http: Client) -> Self {
        self.http = http;
        self
    }

    /// Return the client configuration.
    pub fn config(&self) -> &ProxmoxConfig {
        &self.config
    }

    pub(crate) fn encode(&self, s: &str) -> String {
        urlencoding::encode(s).into_owned()
    }

    fn url(&self, path: &str) -> String {
        let base = self.config.api_base();
        if path.starts_with('/') {
            format!("{base}{path}")
        } else {
            format!("{base}/{path}")
        }
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let (status, body) = self.send(Method::GET, path, None).await?;
        decode_data(status, &body)
    }

    pub(crate) async fn post_form<T: DeserializeOwned>(
        &self,
        path: &str,
        form: &BTreeMap<String, String>,
    ) -> Result<T> {
        let (status, body) = self.send(Method::POST, path, Some(form)).await?;
        decode_data(status, &body)
    }

    pub(crate) async fn put_form<T: DeserializeOwned>(
        &self,
        path: &str,
        form: &BTreeMap<String, String>,
    ) -> Result<T> {
        let (status, body) = self.send(Method::PUT, path, Some(form)).await?;
        decode_data(status, &body)
    }

    pub(crate) async fn delete(&self, path: &str) -> Result<()> {
        let (status, body) = self.send(Method::DELETE, path, None).await?;
        if status.is_success() || status == StatusCode::NO_CONTENT {
            Ok(())
        } else {
            Err(api_error(status.as_u16(), &body))
        }
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        form: Option<&BTreeMap<String, String>>,
    ) -> Result<(StatusCode, String)> {
        let is_write = method != Method::GET;
        let url = self.url(path);
        let mut req = self.http.request(method, url);
        if let Some(form) = form {
            req = req.form(form);
        }
        req = self
            .config
            .auth
            .apply(&self.http, &self.config.api_base(), req, is_write)
            .await?;
        let resp = req.send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        Ok((status, body))
    }
}

/// Proxmox wraps every successful payload in a top-level `{ "data": ... }` object.
#[derive(Deserialize)]
struct DataEnvelope<T> {
    data: T,
}

fn decode_data<T: DeserializeOwned>(status: StatusCode, body: &str) -> Result<T> {
    if status.is_success() {
        let envelope: DataEnvelope<T> = serde_json::from_str(body)?;
        Ok(envelope.data)
    } else {
        Err(api_error(status.as_u16(), body))
    }
}

/// Render a Proxmox error body into a [`ProxmoxError::Api`].
///
/// Proxmox reports failures as `{"data":null,"errors":{...}}` or with a plain
/// `message` field; fall back to the raw body when neither is present.
pub(crate) fn api_error(status: u16, body: &str) -> ProxmoxError {
    #[derive(Deserialize)]
    struct ErrorBody {
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        errors: Option<serde_json::Value>,
    }

    let message = serde_json::from_str::<ErrorBody>(body)
        .ok()
        .and_then(|b| {
            b.message
                .or_else(|| b.errors.filter(|e| !e.is_null()).map(|e| e.to_string()))
        })
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| body.trim().to_string());

    ProxmoxError::Api { status, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> ProxmoxClient {
        ProxmoxClient::new(ProxmoxConfig::new(
            "https://pve.example.com:8006/",
            Auth::api_token("root@pam!ci", "secret"),
        ))
    }

    #[test]
    fn config_trims_host() {
        let cfg = ProxmoxConfig::new(
            "https://pve.example.com:8006/",
            Auth::api_token("root@pam!ci", "s"),
        );
        assert_eq!(cfg.host, "https://pve.example.com:8006");
        assert_eq!(cfg.api_base(), "https://pve.example.com:8006/api2/json");
    }

    #[test]
    fn url_joins_prefix() {
        let c = client();
        assert_eq!(
            c.url("/nodes/pve/qemu"),
            "https://pve.example.com:8006/api2/json/nodes/pve/qemu"
        );
        assert_eq!(
            c.url("nodes/pve/qemu"),
            "https://pve.example.com:8006/api2/json/nodes/pve/qemu"
        );
    }

    #[test]
    fn decode_unwraps_data_envelope() {
        let body = r#"{"data":[{"vmid":100,"name":"web"}]}"#;
        let items: Vec<serde_json::Value> = decode_data(StatusCode::OK, body).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["vmid"], 100);
    }

    #[test]
    fn api_error_prefers_message() {
        let err = api_error(500, r#"{"data":null,"message":"VM 100 not found"}"#);
        assert!(matches!(err, ProxmoxError::Api { status: 500, .. }));
        assert!(err.to_string().contains("VM 100 not found"));
    }

    #[test]
    fn api_error_falls_back_to_body() {
        let err = api_error(401, "authentication failure");
        assert!(err.to_string().contains("authentication failure"));
    }
}
