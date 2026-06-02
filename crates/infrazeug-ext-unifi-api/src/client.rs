//! HTTP client for the UniFi Network controller REST API.

use crate::auth::Credentials;
use crate::error::{Result, UnifiError};
use crate::types::{Meta, UnifiResponse};
use reqwest::header::HeaderMap;
use reqwest::{Client, Method};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Default UniFi site shortname.
pub const DEFAULT_SITE: &str = "default";

/// Which controller flavour we are talking to. This only changes the login URL and
/// the API path prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerKind {
    /// UniFi OS console (UDM/UDM-Pro/Cloud Key Gen2+, self-hosted UniFi OS). The
    /// Network application is reached under `/proxy/network` and login is at
    /// `/api/auth/login`.
    UnifiOs,
    /// Legacy standalone Network controller (`:8443`). Login is at `/api/login` and
    /// the API lives at the host root.
    Legacy,
}

impl ControllerKind {
    /// Parse `"unifios"` / `"legacy"` (case-insensitive); anything else is `UnifiOs`.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "legacy" | "standalone" | "classic" => Self::Legacy,
            _ => Self::UnifiOs,
        }
    }
}

/// Connection configuration for a UniFi controller client.
#[derive(Debug, Clone)]
pub struct UnifiConfig {
    /// Controller base URL, e.g. `https://192.168.1.1` (UniFi OS) or
    /// `https://unifi.example.com:8443` (legacy). No trailing slash.
    pub host: String,
    /// Site shortname (UniFi's internal id, usually `default`).
    pub site: String,
    /// Controller flavour (login URL + path prefix).
    pub controller: ControllerKind,
    /// Authentication material.
    pub credentials: Credentials,
    /// Skip TLS certificate verification for the controller.
    ///
    /// Defaults to `false` (verify). UniFi controllers ship a **self-signed**
    /// certificate out of the box, so against an unmanaged controller you will
    /// typically need [`insecure`](Self::insecure) (or
    /// [`with_accept_invalid_certs(true)`](Self::with_accept_invalid_certs)).
    pub accept_invalid_certs: bool,
}

impl UnifiConfig {
    /// Configuration for a UniFi OS controller with the default site and TLS
    /// certificate verification **enabled**.
    pub fn new(host: impl Into<String>, credentials: Credentials) -> Self {
        Self {
            host: host.into().trim_end_matches('/').to_string(),
            site: DEFAULT_SITE.into(),
            controller: ControllerKind::UnifiOs,
            credentials,
            accept_invalid_certs: false,
        }
    }

    /// Target a non-default site.
    pub fn with_site(mut self, site: impl Into<String>) -> Self {
        self.site = site.into();
        self
    }

    /// Select the controller flavour.
    pub fn with_controller(mut self, controller: ControllerKind) -> Self {
        self.controller = controller;
        self
    }

    /// Toggle acceptance of self-signed / invalid TLS certificates.
    pub fn with_accept_invalid_certs(mut self, accept: bool) -> Self {
        self.accept_invalid_certs = accept;
        self
    }

    /// Skip TLS certificate verification (ignore the controller's self-signed
    /// certificate). Shorthand for `with_accept_invalid_certs(true)`.
    pub fn insecure(self) -> Self {
        self.with_accept_invalid_certs(true)
    }
}

#[derive(Default)]
struct Session {
    logged_in: bool,
    csrf: Option<String>,
}

/// An authenticated UniFi Network controller client.
///
/// The session cookie is managed by the underlying [`reqwest::Client`] cookie
/// store; the rotating CSRF token is tracked here and replayed on mutating
/// requests. Login happens lazily on first use and is shared across clones.
///
/// ```no_run
/// use infrazeug_ext_unifi_api::{Credentials, UnifiClient, UnifiConfig};
///
/// # async fn run() -> infrazeug_ext_unifi_api::Result<()> {
/// let client = UnifiClient::new(
///     UnifiConfig::new(
///         "https://192.168.1.1",
///         Credentials::user_pass("admin", std::env::var("UNIFI_PASSWORD").unwrap()),
///     )
///     .insecure(), // UniFi ships a self-signed cert; skip TLS verification
/// );
/// let wlans = client.wlans().await?;
/// println!("{} WLANs", wlans.len());
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct UnifiClient {
    http: Client,
    config: UnifiConfig,
    session: Arc<RwLock<Session>>,
}

fn build_http(accept_invalid_certs: bool) -> Client {
    Client::builder()
        .cookie_store(true)
        .danger_accept_invalid_certs(accept_invalid_certs)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| Client::new())
}

impl UnifiClient {
    /// Build a client with the given configuration.
    pub fn new(config: UnifiConfig) -> Self {
        let http = build_http(config.accept_invalid_certs);
        Self {
            http,
            config,
            session: Arc::new(RwLock::new(Session::default())),
        }
    }

    /// Use a pre-configured [`reqwest::Client`]. It must have a cookie store
    /// enabled, otherwise session login will not persist.
    pub fn with_http_client(mut self, http: Client) -> Self {
        self.http = http;
        self
    }

    /// The client configuration.
    pub fn config(&self) -> &UnifiConfig {
        &self.config
    }

    pub(crate) fn encode_path(&self, s: &str) -> String {
        urlencoding::encode(s).into_owned()
    }

    /// Root the Network application lives under (adds the `/proxy/network` prefix
    /// on UniFi OS).
    fn api_root(&self) -> String {
        match self.config.controller {
            ControllerKind::UnifiOs => format!("{}/proxy/network", self.config.host),
            ControllerKind::Legacy => self.config.host.clone(),
        }
    }

    fn login_url(&self) -> String {
        match self.config.controller {
            ControllerKind::UnifiOs => format!("{}/api/auth/login", self.config.host),
            ControllerKind::Legacy => format!("{}/api/login", self.config.host),
        }
    }

    /// Path of a site-scoped REST collection (`…/api/s/{site}/rest/{resource}`).
    pub(crate) fn rest_path(&self, resource: &str) -> String {
        format!(
            "{}/api/s/{}/rest/{}",
            self.api_root(),
            self.encode_path(&self.config.site),
            resource
        )
    }

    async fn ensure_session(&self) -> Result<()> {
        if matches!(self.config.credentials, Credentials::ApiKey(_)) {
            return Ok(());
        }
        if self.session.read().await.logged_in {
            return Ok(());
        }
        self.login().await
    }

    async fn login(&self) -> Result<()> {
        let (username, password) = match &self.config.credentials {
            Credentials::UserPass { username, password } => (username, password),
            // API-key auth has no login step.
            Credentials::ApiKey(_) => return Ok(()),
        };

        let mut body = serde_json::json!({ "username": username, "password": password });
        if self.config.controller == ControllerKind::Legacy {
            body["remember"] = serde_json::Value::Bool(true);
            body["strict"] = serde_json::Value::Bool(true);
        }

        let resp = self.http.post(self.login_url()).json(&body).send().await?;
        let status = resp.status();
        let csrf = extract_csrf(resp.headers());
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(UnifiError::Auth(
                api_error(status.as_u16(), &text).to_string(),
            ));
        }

        let mut session = self.session.write().await;
        session.logged_in = true;
        if csrf.is_some() {
            session.csrf = csrf;
        }
        Ok(())
    }

    async fn send<B: Serialize>(
        &self,
        method: Method,
        url: &str,
        body: Option<&B>,
    ) -> Result<reqwest::Response> {
        self.ensure_session().await?;

        let mut req = self.http.request(method, url);
        match &self.config.credentials {
            Credentials::ApiKey(key) => {
                req = req.header("X-API-KEY", key.clone());
            }
            Credentials::UserPass { .. } => {
                let csrf = self.session.read().await.csrf.clone();
                if let Some(csrf) = csrf {
                    req = req.header("X-CSRF-Token", csrf);
                }
            }
        }
        if let Some(body) = body {
            req = req.json(body);
        }

        let resp = req.send().await?;
        // UniFi OS rotates the CSRF token; keep the freshest one for the next call.
        if let Some(csrf) = extract_csrf(resp.headers()) {
            self.session.write().await.csrf = Some(csrf);
        }
        Ok(resp)
    }

    /// `GET …/rest/{resource}` — list a site-scoped REST collection.
    pub(crate) async fn rest_list<T: DeserializeOwned>(&self, resource: &str) -> Result<Vec<T>> {
        let url = self.rest_path(resource);
        let resp = self.send::<()>(Method::GET, &url, None).await?;
        decode_list(resp).await
    }

    /// `POST …/rest/{resource}` — create an object, returning the created records.
    pub(crate) async fn rest_create<B: Serialize, T: DeserializeOwned>(
        &self,
        resource: &str,
        body: &B,
    ) -> Result<Vec<T>> {
        let url = self.rest_path(resource);
        let resp = self.send(Method::POST, &url, Some(body)).await?;
        decode_list(resp).await
    }

    /// `PUT …/rest/{resource}/{id}` — replace an object.
    pub(crate) async fn rest_update<B: Serialize, T: DeserializeOwned>(
        &self,
        resource: &str,
        id: &str,
        body: &B,
    ) -> Result<Vec<T>> {
        let url = format!("{}/{}", self.rest_path(resource), self.encode_path(id));
        let resp = self.send(Method::PUT, &url, Some(body)).await?;
        decode_list(resp).await
    }

    /// `DELETE …/rest/{resource}/{id}` — delete an object.
    pub(crate) async fn rest_delete(&self, resource: &str, id: &str) -> Result<()> {
        let url = format!("{}/{}", self.rest_path(resource), self.encode_path(id));
        let resp = self.send::<()>(Method::DELETE, &url, None).await?;
        let _: Vec<serde_json::Value> = decode_list(resp).await?;
        Ok(())
    }

    /// `GET …/api/self/sites` — sites the authenticated account can administer.
    pub async fn sites(&self) -> Result<Vec<crate::sites::Site>> {
        let url = format!("{}/api/self/sites", self.api_root());
        let resp = self.send::<()>(Method::GET, &url, None).await?;
        decode_list(resp).await
    }

    /// Path of a site-scoped **v2** API endpoint (`…/v2/api/site/{site}/{suffix}`).
    ///
    /// The newer v2 API (used by DNS records, traffic rules, …) returns payloads
    /// *unwrapped* — a bare JSON array or object rather than the `{ meta, data }`
    /// envelope of the classic REST API.
    pub(crate) fn v2_path(&self, suffix: &str) -> String {
        format!(
            "{}/v2/api/site/{}/{}",
            self.api_root(),
            self.encode_path(&self.config.site),
            suffix
        )
    }

    /// `GET …/v2/api/site/{site}/{suffix}` — list a v2 collection.
    pub(crate) async fn v2_list<T: DeserializeOwned>(&self, suffix: &str) -> Result<Vec<T>> {
        let url = self.v2_path(suffix);
        let resp = self.send::<()>(Method::GET, &url, None).await?;
        decode_v2(resp).await
    }

    /// `POST …/v2/api/site/{site}/{suffix}` — create a v2 object.
    pub(crate) async fn v2_create<B: Serialize, T: DeserializeOwned>(
        &self,
        suffix: &str,
        body: &B,
    ) -> Result<T> {
        let url = self.v2_path(suffix);
        let resp = self.send(Method::POST, &url, Some(body)).await?;
        decode_v2(resp).await
    }

    /// `PUT …/v2/api/site/{site}/{suffix}` — replace a v2 object (`suffix` carries the id).
    pub(crate) async fn v2_update<B: Serialize, T: DeserializeOwned>(
        &self,
        suffix: &str,
        body: &B,
    ) -> Result<T> {
        let url = self.v2_path(suffix);
        let resp = self.send(Method::PUT, &url, Some(body)).await?;
        decode_v2(resp).await
    }

    /// `DELETE …/v2/api/site/{site}/{suffix}` — delete a v2 object.
    pub(crate) async fn v2_delete(&self, suffix: &str) -> Result<()> {
        let url = self.v2_path(suffix);
        let resp = self.send::<()>(Method::DELETE, &url, None).await?;
        let status = resp.status();
        let text = resp.text().await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(v2_error(status.as_u16(), &text))
        }
    }

    /// Path of a site-scoped **stat** endpoint (`…/api/s/{site}/stat/{suffix}`).
    ///
    /// Stat endpoints are read-only live state (devices, clients, health, …) and
    /// reuse the same `{ meta, data }` envelope as the classic REST API.
    pub(crate) fn stat_path(&self, suffix: &str) -> String {
        format!(
            "{}/api/s/{}/stat/{}",
            self.api_root(),
            self.encode_path(&self.config.site),
            suffix
        )
    }

    /// `GET …/stat/{suffix}` — read a site-scoped stat collection.
    pub(crate) async fn stat_list<T: DeserializeOwned>(&self, suffix: &str) -> Result<Vec<T>> {
        let url = self.stat_path(suffix);
        let resp = self.send::<()>(Method::GET, &url, None).await?;
        decode_list(resp).await
    }

    /// Path of a site-scoped **command** manager (`…/api/s/{site}/cmd/{manager}`).
    pub(crate) fn cmd_path(&self, manager: &str) -> String {
        format!(
            "{}/api/s/{}/cmd/{}",
            self.api_root(),
            self.encode_path(&self.config.site),
            manager
        )
    }

    /// `POST …/cmd/{manager}` — invoke an imperative command (device restart,
    /// client block, …). Any returned `data` is validated for `rc == ok` and
    /// discarded.
    pub(crate) async fn cmd<B: Serialize>(&self, manager: &str, body: &B) -> Result<()> {
        let url = self.cmd_path(manager);
        let resp = self.send(Method::POST, &url, Some(body)).await?;
        let _: Vec<serde_json::Value> = decode_list(resp).await?;
        Ok(())
    }
}

/// Take the first item, mapping an empty payload to an error (`create`/`update`
/// always echo the affected object back).
pub(crate) fn first_item<T>(items: Vec<T>, what: &str) -> Result<T> {
    items.into_iter().next().ok_or_else(|| UnifiError::Api {
        status: 200,
        message: format!("unifi returned no {what}"),
    })
}

fn extract_csrf(headers: &HeaderMap) -> Option<String> {
    for name in ["x-updated-csrf-token", "x-csrf-token"] {
        if let Some(value) = headers.get(name) {
            if let Ok(s) = value.to_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

async fn decode_list<T: DeserializeOwned>(resp: reqwest::Response) -> Result<Vec<T>> {
    let status = resp.status();
    let text = resp.text().await?;
    match serde_json::from_str::<UnifiResponse<T>>(&text) {
        Ok(env) if env.meta.is_ok() => Ok(env.data),
        Ok(env) => {
            let message = env
                .meta
                .msg
                .clone()
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| envelope_rc(&env.meta));
            Err(UnifiError::Api {
                status: status.as_u16(),
                message,
            })
        }
        Err(json_err) => {
            if status.is_success() {
                Err(UnifiError::Json(json_err))
            } else {
                Err(api_error(status.as_u16(), &text))
            }
        }
    }
}

fn envelope_rc(meta: &Meta) -> String {
    if meta.rc.is_empty() {
        "unifi request failed".into()
    } else {
        meta.rc.clone()
    }
}

/// Decode a v2 API response (bare JSON, no `{ meta, data }` envelope).
async fn decode_v2<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
    let status = resp.status();
    let text = resp.text().await?;
    if status.is_success() {
        serde_json::from_str(&text).map_err(UnifiError::Json)
    } else {
        Err(v2_error(status.as_u16(), &text))
    }
}

fn v2_error(status: u16, body: &str) -> UnifiError {
    #[derive(serde::Deserialize)]
    struct V2Error {
        message: Option<String>,
        #[serde(rename = "errorCode")]
        error_code: Option<String>,
    }

    let message = serde_json::from_str::<V2Error>(body)
        .ok()
        .and_then(|e| e.message.or(e.error_code))
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| body.trim().to_string());
    UnifiError::Api { status, message }
}

pub(crate) fn api_error(status: u16, body: &str) -> UnifiError {
    #[derive(serde::Deserialize)]
    struct ErrorEnvelope {
        meta: Option<Meta>,
    }

    let message = serde_json::from_str::<ErrorEnvelope>(body)
        .ok()
        .and_then(|e| e.meta)
        .and_then(|m| {
            m.msg
                .filter(|s| !s.is_empty())
                .or((!m.rc.is_empty()).then_some(m.rc))
        })
        .unwrap_or_else(|| body.trim().to_string());
    UnifiError::Api { status, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(kind: ControllerKind) -> UnifiClient {
        UnifiClient::new(
            UnifiConfig::new(
                "https://unifi.local/",
                Credentials::user_pass("admin", "pw"),
            )
            .with_controller(kind),
        )
    }

    #[test]
    fn config_trims_host() {
        let cfg = UnifiConfig::new("https://unifi.local/", Credentials::api_key("k"));
        assert_eq!(cfg.host, "https://unifi.local");
        assert_eq!(cfg.site, DEFAULT_SITE);
        // TLS verification is on by default; opt in to ignoring the cert.
        assert!(!cfg.accept_invalid_certs);
        assert!(cfg.insecure().accept_invalid_certs);
    }

    #[test]
    fn unifi_os_paths_use_proxy_prefix() {
        let c = client(ControllerKind::UnifiOs);
        assert_eq!(
            c.rest_path("wlanconf"),
            "https://unifi.local/proxy/network/api/s/default/rest/wlanconf"
        );
        assert_eq!(c.login_url(), "https://unifi.local/api/auth/login");
    }

    #[test]
    fn legacy_paths_use_host_root() {
        let c = client(ControllerKind::Legacy);
        assert_eq!(
            c.rest_path("networkconf"),
            "https://unifi.local/api/s/default/rest/networkconf"
        );
        assert_eq!(c.login_url(), "https://unifi.local/api/login");
    }

    #[test]
    fn stat_and_cmd_paths() {
        let c = client(ControllerKind::UnifiOs);
        assert_eq!(
            c.stat_path("device"),
            "https://unifi.local/proxy/network/api/s/default/stat/device"
        );
        assert_eq!(
            c.cmd_path("devmgr"),
            "https://unifi.local/proxy/network/api/s/default/cmd/devmgr"
        );
    }

    #[test]
    fn v2_paths_use_site_segment() {
        assert_eq!(
            client(ControllerKind::UnifiOs).v2_path("static-dns"),
            "https://unifi.local/proxy/network/v2/api/site/default/static-dns"
        );
        assert_eq!(
            client(ControllerKind::Legacy).v2_path("static-dns"),
            "https://unifi.local/v2/api/site/default/static-dns"
        );
    }

    #[test]
    fn site_is_url_encoded() {
        let c = UnifiClient::new(
            UnifiConfig::new("https://unifi.local", Credentials::api_key("k")).with_site("my site"),
        );
        assert!(c
            .rest_path("portforward")
            .contains("/api/s/my%20site/rest/"));
    }

    #[test]
    fn controller_kind_parses() {
        assert_eq!(ControllerKind::parse("legacy"), ControllerKind::Legacy);
        assert_eq!(ControllerKind::parse("UniFiOS"), ControllerKind::UnifiOs);
        assert_eq!(ControllerKind::parse("anything"), ControllerKind::UnifiOs);
    }

    #[test]
    fn api_error_prefers_meta_msg() {
        let err = api_error(400, r#"{"meta":{"rc":"error","msg":"api.err.Invalid"}}"#);
        match err {
            UnifiError::Api { status, message } => {
                assert_eq!(status, 400);
                assert_eq!(message, "api.err.Invalid");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
