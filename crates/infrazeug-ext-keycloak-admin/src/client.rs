//! Bearer-token HTTP client for the Keycloak Admin REST API.
//!
//! [`KeycloakClient`] handles base URL construction, transparent token
//! acquisition and refresh via OAuth2, and exposes typed HTTP verbs used by the
//! resource modules ([`crate::realms`], [`crate::users`], …).

use crate::error::{KeycloakError, Result};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// OAuth2 grant type used to obtain an admin access token.
#[derive(Debug, Clone)]
pub enum GrantType {
    /// `grant_type=client_credentials` — machine-to-machine.
    ClientCredentials {
        client_id: String,
        client_secret: String,
    },
    /// `grant_type=password` — admin-cli style.
    Password {
        client_id: String,
        client_secret: Option<String>,
        username: String,
        password: String,
    },
}

/// Connection configuration for a Keycloak Admin client.
#[derive(Debug, Clone)]
pub struct KeycloakConfig {
    /// Keycloak base URL (e.g. `https://keycloak.example.local`). No trailing slash.
    pub base_url: String,
    /// Realm used for token acquisition and default target realm.
    pub realm: String,
    /// Grant type used to obtain access tokens.
    pub grant_type: GrantType,
}

impl KeycloakConfig {
    /// Create a new configuration.
    pub fn new(
        base_url: impl Into<String>,
        realm: impl Into<String>,
        grant_type: GrantType,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            realm: realm.into(),
            grant_type,
        }
    }
}

#[derive(Debug)]
pub(crate) struct TokenState {
    access_token: String,
    expires_at: tokio::time::Instant,
}

/// An authenticated Keycloak Admin REST API client.
///
/// ```no_run
/// use infrazeug_ext_keycloak_admin::{KeycloakClient, KeycloakConfig, GrantType};
///
/// # async fn run() -> infrazeug_ext_keycloak_admin::Result<()> {
/// let config = KeycloakConfig::new(
///     "https://keycloak.example.local",
///     "master",
///     GrantType::ClientCredentials {
///         client_id: "admin-cli".into(),
///         client_secret: "secret".into(),
///     },
/// );
/// let client = KeycloakClient::new(config);
/// let realms = client.realms().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct KeycloakClient {
    pub(crate) http: Client,
    pub(crate) config: KeycloakConfig,
    pub(crate) token: Arc<RwLock<Option<TokenState>>>,
}

/// Build the default HTTP client for the Admin API.
///
/// - **System trust store** (as the OS and `curl` use), in addition to the bundled webpki
///   roots. Internal Keycloak deployments are often fronted by a private CA; rustls'
///   webpki-only default rejects those certs, surfacing as an opaque "error sending request"
///   transport error during token acquisition.
/// - **HTTP/1.1 only.** Admin calls token-then-act over a pooled connection; reusing an
///   HTTP/2 connection through some ingresses/proxies can stall indefinitely. h1 keep-alive is
///   far more robust through proxies and the admin REST API needs nothing h2-specific.
/// - **Bounded timeouts.** Without them any stall is an infinite hang that blocks the whole
///   apply; a connect/overall deadline turns it into a clear, retryable error instead.
///
/// Falls back to the plain client if the builder fails (e.g. the native store cannot load).
fn default_http_client() -> Client {
    Client::builder()
        .tls_built_in_native_certs(true)
        .http1_only()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| Client::new())
}

impl KeycloakClient {
    /// Build a client with the given configuration.
    pub fn new(config: KeycloakConfig) -> Self {
        Self {
            http: default_http_client(),
            config,
            token: Arc::new(RwLock::new(None)),
        }
    }

    /// Use a pre-configured [`reqwest::Client`] (custom timeouts, proxy, …).
    pub fn with_http_client(mut self, http: Client) -> Self {
        self.http = http;
        self
    }

    pub(crate) fn admin_url(&self, path: &str) -> String {
        let mut url = self.config.base_url.clone();
        url.push_str("/admin/realms");
        if !path.starts_with('/') {
            url.push('/');
        }
        url.push_str(path);
        url
    }

    pub(crate) async fn ensure_token(&self) -> Result<()> {
        {
            let guard = self.token.read().await;
            if let Some(state) = guard.as_ref() {
                if state.expires_at > tokio::time::Instant::now() {
                    return Ok(());
                }
            }
        }
        self.refresh_token().await
    }

    async fn refresh_token(&self) -> Result<()> {
        let token_url = format!(
            "{}/realms/{}/protocol/openid-connect/token",
            self.config.base_url, self.config.realm
        );

        let mut params = vec![("grant_type", self.grant_type_name().to_string())];

        match &self.config.grant_type {
            GrantType::ClientCredentials {
                client_id,
                client_secret,
            } => {
                params.push(("client_id", client_id.clone()));
                params.push(("client_secret", client_secret.clone()));
            }
            GrantType::Password {
                client_id,
                client_secret,
                username,
                password,
            } => {
                params.push(("client_id", client_id.clone()));
                if let Some(secret) = client_secret {
                    params.push(("client_secret", secret.clone()));
                }
                params.push(("username", username.clone()));
                params.push(("password", password.clone()));
            }
        }

        let resp = self
            .http
            .post(&token_url)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await?;

        if !status.is_success() {
            return Err(KeycloakError::Auth(format!(
                "token endpoint returned {status}: {body}"
            )));
        }

        #[derive(serde::Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: Option<u64>,
        }

        let token: TokenResponse = serde_json::from_str(&body)?;
        let expires_in = token.expires_in.unwrap_or(300);
        let state = TokenState {
            access_token: token.access_token,
            expires_at: tokio::time::Instant::now()
                + tokio::time::Duration::from_secs(expires_in - 10),
        };

        let mut guard = self.token.write().await;
        *guard = Some(state);
        Ok(())
    }

    fn grant_type_name(&self) -> &str {
        match &self.config.grant_type {
            GrantType::ClientCredentials { .. } => "client_credentials",
            GrantType::Password { .. } => "password",
        }
    }

    async fn send_request(
        &self,
        method: Method,
        url: &str,
        body: Option<&str>,
    ) -> Result<reqwest::Response> {
        self.ensure_token().await?;
        let guard = self.token.read().await;
        let token = guard
            .as_ref()
            .expect("token must be set after ensure_token")
            .access_token
            .clone();
        drop(guard);

        let mut req = self
            .http
            .request(method, url)
            .header(AUTHORIZATION, format!("Bearer {token}"));

        if let Some(body) = body {
            req = req
                .header(CONTENT_TYPE, "application/json")
                .body(body.to_string());
        }

        let resp = req.send().await?;
        Ok(resp)
    }

    pub(crate) fn encode_path(&self, s: &str) -> String {
        urlencoding::encode(s).into_owned()
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = self.admin_url(path);
        let resp = self.send_request(Method::GET, &url, None).await?;
        let (status, body) = consume(resp).await?;
        decode(status, &body)
    }

    pub(crate) async fn get_with_query<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T> {
        let url = self.admin_url(path);
        let url = append_query(&url, query);
        let resp = self.send_request(Method::GET, &url, None).await?;
        let (status, body) = consume(resp).await?;
        decode(status, &body)
    }

    pub(crate) async fn get_raw(&self, path: &str) -> Result<String> {
        let url = self.admin_url(path);
        let resp = self.send_request(Method::GET, &url, None).await?;
        let (_, body) = consume(resp).await?;
        Ok(body)
    }

    #[allow(dead_code)]
    pub(crate) async fn get_optional<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>> {
        let url = self.admin_url(path);
        let resp = self.send_request(Method::GET, &url, None).await?;
        let status = resp.status();
        let body = resp.text().await?;
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if status.is_success() {
            let val: T = serde_json::from_str(&body)?;
            return Ok(Some(val));
        }
        Err(api_error(status.as_u16(), &body))
    }

    pub(crate) async fn post<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<()> {
        let url = self.admin_url(path);
        let body = serde_json::to_string(body)?;
        let resp = self.send_request(Method::POST, &url, Some(&body)).await?;
        let (status, text) = consume(resp).await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(api_error(status.as_u16(), &text))
        }
    }

    pub(crate) async fn post_and_extract_id<B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<String> {
        let url = self.admin_url(path);
        let body_str = serde_json::to_string(body)?;
        self.ensure_token().await?;
        let guard = self.token.read().await;
        let token = guard.as_ref().expect("token set").access_token.clone();
        drop(guard);

        let resp = self
            .http
            .request(Method::POST, &url)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header(CONTENT_TYPE, "application/json")
            .body(body_str)
            .send()
            .await?;

        let status = resp.status();
        if status == StatusCode::CREATED || status.is_success() {
            if let Some(location) = resp.headers().get("location") {
                let loc = location
                    .to_str()
                    .map_err(|e| KeycloakError::Auth(e.to_string()))?;
                if let Some(id) = loc.rsplit('/').next() {
                    return Ok(id.to_string());
                }
            }
            let text = resp.text().await?;
            if !text.is_empty() {
                if let Ok(id) = serde_json::from_str::<String>(&text) {
                    return Ok(id);
                }
            }
            Ok(String::new())
        } else {
            let text = resp.text().await?;
            Err(api_error(status.as_u16(), &text))
        }
    }

    pub(crate) async fn put<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<()> {
        let url = self.admin_url(path);
        let body = serde_json::to_string(body)?;
        let resp = self.send_request(Method::PUT, &url, Some(&body)).await?;
        let (status, text) = consume(resp).await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(api_error(status.as_u16(), &text))
        }
    }

    pub(crate) async fn delete(&self, path: &str) -> Result<()> {
        let url = self.admin_url(path);
        let resp = self.send_request(Method::DELETE, &url, None).await?;
        let (status, text) = consume(resp).await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(api_error(status.as_u16(), &text))
        }
    }

    pub(crate) async fn put_json<B, T>(&self, path: &str, body: &B) -> Result<T>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let url = self.admin_url(path);
        let body = serde_json::to_string(body)?;
        let resp = self.send_request(Method::PUT, &url, Some(&body)).await?;
        let (status, text) = consume(resp).await?;
        decode(status, &text)
    }

    pub(crate) async fn request_raw(
        &self,
        method: Method,
        path: &str,
        body: Option<&str>,
    ) -> Result<(StatusCode, String)> {
        let url = self.admin_url(path);
        let resp = self.send_request(method, &url, body).await?;
        consume(resp).await
    }
}

async fn consume(resp: reqwest::Response) -> Result<(StatusCode, String)> {
    let status = resp.status();
    let body = resp.text().await?;
    Ok((status, body))
}

fn decode<T: DeserializeOwned>(status: StatusCode, body: &str) -> Result<T> {
    if status.is_success() {
        Ok(serde_json::from_str(body)?)
    } else {
        Err(api_error(status.as_u16(), body))
    }
}

fn api_error(status: u16, body: &str) -> KeycloakError {
    #[derive(serde::Deserialize)]
    struct ErrorBody {
        error: Option<String>,
        error_description: Option<String>,
        #[serde(rename = "errorMessage")]
        error_message: Option<String>,
    }

    let parsed: Option<ErrorBody> = serde_json::from_str(body).ok();
    let message = parsed
        .as_ref()
        .and_then(|b| b.error_description.clone().or(b.error_message.clone()))
        .unwrap_or_else(|| body.trim().to_string());
    let error = parsed.and_then(|b| b.error);
    KeycloakError::Api {
        status,
        error,
        message,
    }
}

fn append_query(url: &str, query: &[(&str, &str)]) -> String {
    if query.is_empty() {
        return url.to_string();
    }
    let mut out = url.to_string();
    out.push('?');
    for (i, (k, v)) in query.iter().enumerate() {
        if i > 0 {
            out.push('&');
        }
        out.push_str(&urlencoding::encode(k));
        out.push('=');
        out.push_str(&urlencoding::encode(v));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_trims_trailing_slash() {
        let c = KeycloakConfig::new(
            "http://kc.local/",
            "master",
            GrantType::ClientCredentials {
                client_id: "a".into(),
                client_secret: "b".into(),
            },
        );
        assert_eq!(c.base_url, "http://kc.local");
    }

    #[test]
    fn admin_url_joins_correctly() {
        let c = KeycloakClient::new(KeycloakConfig::new(
            "http://kc.local",
            "master",
            GrantType::ClientCredentials {
                client_id: "a".into(),
                client_secret: "b".into(),
            },
        ));
        assert_eq!(
            c.admin_url("/master/users"),
            "http://kc.local/admin/realms/master/users"
        );
        assert_eq!(
            c.admin_url("master/users"),
            "http://kc.local/admin/realms/master/users"
        );
    }

    #[test]
    fn append_query_encodes() {
        let url = append_query("http://x/y", &[("search", "a b"), ("brief", "true")]);
        assert_eq!(url, "http://x/y?search=a%20b&brief=true");
    }
}
