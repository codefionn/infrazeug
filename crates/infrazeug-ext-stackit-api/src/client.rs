//! HTTP client for the STACKIT IaaS API.

use crate::auth::Auth;
use crate::error::{Result, StackitError};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub(crate) const DEFAULT_HOST: &str = "https://iaas.api.eu01.stackit.cloud";
pub(crate) const DEFAULT_API_VERSION: &str = "v1";

/// Connection configuration for a STACKIT IaaS API client.
#[derive(Debug, Clone)]
pub struct StackitConfig {
    /// Regional IaaS API host (default `https://iaas.api.eu01.stackit.cloud`).
    pub host: String,
    /// API version path segment (`v1` or `v2`).
    pub api_version: String,
    /// Authentication method (token or service-account key flow).
    pub auth: Auth,
}

impl StackitConfig {
    /// Create a configuration with the default EU01 production host and API v1.
    pub fn new(auth: Auth) -> Self {
        Self {
            host: DEFAULT_HOST.into(),
            api_version: DEFAULT_API_VERSION.into(),
            auth,
        }
    }

    /// Override the regional API host (e.g. `https://iaas.api.eu01.stackit.cloud`).
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into().trim_end_matches('/').to_string();
        self
    }

    /// Select the API version (`v1` or `v2`).
    pub fn with_api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = version.into();
        self
    }
}

/// An authenticated STACKIT IaaS API client.
#[derive(Clone)]
pub struct StackitClient {
    http: Client,
    config: StackitConfig,
}

impl StackitClient {
    /// Build a client with the given configuration.
    pub fn new(config: StackitConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    /// Use a pre-configured [`reqwest::Client`] (custom timeouts, proxy, …).
    pub fn with_http_client(mut self, http: Client) -> Self {
        self.http = http;
        self
    }

    /// Return a clone of the client configuration.
    pub fn config(&self) -> &StackitConfig {
        &self.config
    }

    pub(crate) fn encode_path(&self, s: &str) -> String {
        urlencoding::encode(s).into_owned()
    }

    /// Build a project-scoped API path.
    pub(crate) fn project_path(&self, project_id: &str, suffix: &str) -> String {
        let mut path = format!(
            "/{}/projects/{}",
            self.config.api_version,
            self.encode_path(project_id)
        );
        if !suffix.is_empty() {
            if !suffix.starts_with('/') {
                path.push('/');
            }
            path.push_str(suffix.trim_start_matches('/'));
        }
        path
    }

    /// Build a v2 project + region scoped API path.
    pub(crate) fn regional_path(&self, project_id: &str, region: &str, suffix: &str) -> String {
        let mut path = format!(
            "/v2/projects/{}/regions/{}",
            self.encode_path(project_id),
            self.encode_path(region)
        );
        if !suffix.is_empty() {
            if !suffix.starts_with('/') {
                path.push('/');
            }
            path.push_str(suffix.trim_start_matches('/'));
        }
        path
    }

    fn base_url(&self, path: &str) -> String {
        let mut url = self.config.host.trim_end_matches('/').to_string();
        if !path.starts_with('/') {
            url.push('/');
        }
        url.push_str(path);
        url
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self.send(Method::GET, path, None::<&()>).await?;
        let (status, body) = consume(resp).await?;
        decode(status, &body)
    }

    pub(crate) async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let resp = self.send(Method::POST, path, Some(body)).await?;
        let (status, text) = consume(resp).await?;
        decode(status, &text)
    }

    pub(crate) async fn delete(&self, path: &str) -> Result<()> {
        let resp = self.send(Method::DELETE, path, None::<&()>).await?;
        let (status, body) = consume(resp).await?;
        if status.is_success() || status == StatusCode::NO_CONTENT {
            Ok(())
        } else {
            Err(api_error(status.as_u16(), &body))
        }
    }

    async fn send<B: Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<reqwest::Response> {
        let url = self.base_url(path);
        let mut headers = HeaderMap::new();
        let auth = self.config.auth.authorization_header().await?;
        headers.insert(AUTHORIZATION, auth);
        let mut req = self.http.request(method, url);
        if let Some(body) = body {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            let json = serde_json::to_string(body)?;
            req = req.headers(headers).body(json);
        } else {
            req = req.headers(headers);
        }
        Ok(req.send().await?)
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

pub(crate) fn api_error(status: u16, body: &str) -> StackitError {
    #[derive(serde::Deserialize)]
    struct ErrorBody {
        message: Option<String>,
        #[serde(default)]
        detail: Option<String>,
    }

    let parsed: Option<ErrorBody> = serde_json::from_str(body).ok();
    let message = parsed
        .and_then(|b| b.message.or(b.detail))
        .unwrap_or_else(|| body.trim().to_string());

    StackitError::Api { status, message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Auth;

    #[test]
    fn config_trims_host() {
        let cfg =
            StackitConfig::new(Auth::token("t")).with_host("https://iaas.api.eu01.stackit.cloud/");
        assert_eq!(cfg.host, "https://iaas.api.eu01.stackit.cloud");
    }

    #[test]
    fn project_path_joins() {
        let client = StackitClient::new(StackitConfig::new(Auth::token("t")));
        assert_eq!(
            client.project_path("proj-1", "servers"),
            "/v1/projects/proj-1/servers"
        );
    }

    #[test]
    fn regional_path_joins() {
        let client = StackitClient::new(StackitConfig::new(Auth::token("t")));
        assert_eq!(
            client.regional_path("proj-1", "eu01", "servers"),
            "/v2/projects/proj-1/regions/eu01/servers"
        );
    }
}
