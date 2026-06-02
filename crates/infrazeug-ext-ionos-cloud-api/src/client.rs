//! HTTP client for the IONOS Cloud API v6 and Auth API v1.

use crate::auth::Auth;
use crate::error::{IonosError, Result};
use crate::types::{ApiInfo, ListQuery};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub(crate) const DEFAULT_HOST: &str = "https://api.ionos.com";
pub(crate) const CLOUD_API_PATH: &str = "/cloudapi/v6";
pub(crate) const AUTH_API_PATH: &str = "/auth/v1";

/// Connection configuration for an IONOS Cloud API client.
#[derive(Debug, Clone)]
pub struct IonosConfig {
    /// API host (default `https://api.ionos.com`). Trailing slashes are stripped.
    pub host: String,
    /// Authentication method (Bearer token or Basic credentials).
    pub auth: Auth,
    /// Contract number for users with multiple contracts (`X-Contract-Number`).
    pub contract_number: Option<String>,
}

impl IonosConfig {
    /// Create a configuration with the default production host.
    pub fn new(auth: Auth) -> Self {
        Self {
            host: DEFAULT_HOST.into(),
            auth,
            contract_number: None,
        }
    }

    /// Override the API host (e.g. `https://api.ionos.com`).
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into().trim_end_matches('/').to_string();
        self
    }

    /// Set the contract number header for multi-contract accounts.
    pub fn with_contract_number(mut self, contract_number: impl Into<String>) -> Self {
        self.contract_number = Some(contract_number.into());
        self
    }
}

/// An authenticated IONOS Cloud API client.
///
/// ```no_run
/// use infrazeug_ext_ionos_cloud_api::{Auth, IonosClient, IonosConfig};
///
/// # async fn run() -> infrazeug_ext_ionos_cloud_api::Result<()> {
/// let client = IonosClient::new(IonosConfig::new(Auth::token(
///     std::env::var("IONOS_TOKEN").unwrap(),
/// )));
///
/// let info = client.api_info().await?;
/// println!("{} v{}", info.name.unwrap_or_default(), info.version.unwrap_or_default());
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct IonosClient {
    http: Client,
    config: IonosConfig,
}

impl IonosClient {
    /// Build a client with the given configuration.
    pub fn new(config: IonosConfig) -> Self {
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
    pub fn config(&self) -> &IonosConfig {
        &self.config
    }

    pub(crate) fn cloud_url(&self, path: &str) -> String {
        join_url(&self.config.host, CLOUD_API_PATH, path)
    }

    pub(crate) fn auth_url(&self, path: &str) -> String {
        join_url(&self.config.host, AUTH_API_PATH, path)
    }

    pub(crate) fn encode_path(&self, s: &str) -> String {
        urlencoding::encode(s).into_owned()
    }

    fn apply_auth(&self, headers: &mut HeaderMap) -> Result<()> {
        let auth = self.config.auth.authorization_header()?;
        headers.insert(AUTHORIZATION, auth);
        if let Some(contract) = &self.config.contract_number {
            let value =
                HeaderValue::from_str(contract).map_err(|e| IonosError::Auth(e.to_string()))?;
            headers.insert("X-Contract-Number", value);
        }
        Ok(())
    }

    pub(crate) async fn send_cloud(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&str>,
        if_match: Option<&str>,
    ) -> Result<reqwest::Response> {
        let url = append_query(&self.cloud_url(path), query);
        self.send(method, &url, body, if_match).await
    }

    pub(crate) async fn send_auth(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&str>,
    ) -> Result<reqwest::Response> {
        let url = append_query(&self.auth_url(path), query);
        self.send(method, &url, body, None).await
    }

    async fn send(
        &self,
        method: Method,
        url: &str,
        body: Option<&str>,
        if_match: Option<&str>,
    ) -> Result<reqwest::Response> {
        let mut headers = HeaderMap::new();
        self.apply_auth(&mut headers)?;
        if body.is_some() {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }
        if let Some(etag) = if_match {
            let value = HeaderValue::from_str(etag).map_err(|e| IonosError::Auth(e.to_string()))?;
            headers.insert("If-Match", value);
        }

        let mut req = self.http.request(method, url).headers(headers);
        if let Some(body) = body {
            req = req.body(body.to_string());
        }
        Ok(req.send().await?)
    }

    pub(crate) async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &ListQuery,
    ) -> Result<T> {
        let params = query.as_params();
        let resp = self
            .send_cloud(Method::GET, path, &params, None, None)
            .await?;
        let (status, body) = consume(resp).await?;
        decode(status, &body)
    }

    pub(crate) async fn get_auth<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let resp = self.send_auth(Method::GET, path, query, None).await?;
        let (status, body) = consume(resp).await?;
        decode(status, &body)
    }

    pub(crate) async fn post<B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
        query: &ListQuery,
    ) -> Result<()> {
        let body = serde_json::to_string(body)?;
        let params = query.as_params();
        let resp = self
            .send_cloud(Method::POST, path, &params, Some(&body), None)
            .await?;
        let (status, text) = consume(resp).await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(api_error(status.as_u16(), &text))
        }
    }

    pub(crate) async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        query: &ListQuery,
    ) -> Result<T> {
        let body = serde_json::to_string(body)?;
        let params = query.as_params();
        let resp = self
            .send_cloud(Method::POST, path, &params, Some(&body), None)
            .await?;
        let (status, text) = consume(resp).await?;
        decode(status, &text)
    }

    #[allow(dead_code)]
    pub(crate) async fn put<B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
        query: &ListQuery,
        if_match: Option<&str>,
    ) -> Result<()> {
        let body = serde_json::to_string(body)?;
        let params = query.as_params();
        let resp = self
            .send_cloud(Method::PUT, path, &params, Some(&body), if_match)
            .await?;
        let (status, text) = consume(resp).await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(api_error(status.as_u16(), &text))
        }
    }

    pub(crate) async fn put_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        query: &ListQuery,
        if_match: Option<&str>,
    ) -> Result<T> {
        let body = serde_json::to_string(body)?;
        let params = query.as_params();
        let resp = self
            .send_cloud(Method::PUT, path, &params, Some(&body), if_match)
            .await?;
        let (status, text) = consume(resp).await?;
        decode(status, &text)
    }

    #[allow(dead_code)]
    pub(crate) async fn patch<B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
        query: &ListQuery,
        if_match: Option<&str>,
    ) -> Result<()> {
        let body = serde_json::to_string(body)?;
        let params = query.as_params();
        let resp = self
            .send_cloud(Method::PATCH, path, &params, Some(&body), if_match)
            .await?;
        let (status, text) = consume(resp).await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(api_error(status.as_u16(), &text))
        }
    }

    pub(crate) async fn delete(
        &self,
        path: &str,
        query: &ListQuery,
        if_match: Option<&str>,
    ) -> Result<()> {
        let params = query.as_params();
        let resp = self
            .send_cloud(Method::DELETE, path, &params, None, if_match)
            .await?;
        let (status, text) = consume(resp).await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(api_error(status.as_u16(), &text))
        }
    }

    /// `GET /` — retrieve Cloud API version information.
    pub async fn api_info(&self) -> Result<ApiInfo> {
        self.get("", &ListQuery::default()).await
    }
}

fn join_url(host: &str, base_path: &str, path: &str) -> String {
    let mut url = host.trim_end_matches('/').to_string();
    url.push_str(base_path);
    if !path.is_empty() {
        if !path.starts_with('/') {
            url.push('/');
        }
        url.push_str(path);
    }
    url
}

fn append_query(url: &str, query: &[(&str, String)]) -> String {
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

pub(crate) async fn consume(resp: reqwest::Response) -> Result<(StatusCode, String)> {
    let status = resp.status();
    let body = resp.text().await?;
    Ok((status, body))
}

pub(crate) fn decode<T: DeserializeOwned>(status: StatusCode, body: &str) -> Result<T> {
    if status.is_success() {
        Ok(serde_json::from_str(body)?)
    } else {
        Err(api_error(status.as_u16(), body))
    }
}

pub(crate) fn api_error(status: u16, body: &str) -> IonosError {
    #[derive(serde::Deserialize)]
    struct ErrorBody {
        #[serde(rename = "httpStatus")]
        http_status: Option<u16>,
        messages: Option<Vec<ErrorMessage>>,
    }

    #[derive(serde::Deserialize)]
    struct ErrorMessage {
        #[serde(rename = "errorCode")]
        error_code: Option<String>,
        message: Option<String>,
    }

    let parsed: Option<ErrorBody> = serde_json::from_str(body).ok();
    let codes: Vec<String> = parsed
        .as_ref()
        .and_then(|b| b.messages.as_ref())
        .map(|msgs| msgs.iter().filter_map(|m| m.error_code.clone()).collect())
        .unwrap_or_default();
    let message = parsed
        .as_ref()
        .and_then(|b| b.messages.as_ref())
        .and_then(|msgs| {
            msgs.iter()
                .filter_map(|m| m.message.clone())
                .reduce(|a, b| format!("{a}; {b}"))
        })
        .unwrap_or_else(|| body.trim().to_string());
    let status = parsed.and_then(|b| b.http_status).unwrap_or(status);

    IonosError::Api {
        status,
        codes,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_trims_host() {
        let cfg = IonosConfig::new(Auth::token("t")).with_host("https://api.ionos.com/");
        assert_eq!(cfg.host, "https://api.ionos.com");
    }

    #[test]
    fn cloud_url_joins() {
        let client = IonosClient::new(IonosConfig::new(Auth::token("t")));
        assert_eq!(
            client.cloud_url("/datacenters"),
            "https://api.ionos.com/cloudapi/v6/datacenters"
        );
        assert_eq!(
            client.cloud_url("datacenters"),
            "https://api.ionos.com/cloudapi/v6/datacenters"
        );
    }

    #[test]
    fn auth_url_joins() {
        let client = IonosClient::new(IonosConfig::new(Auth::token("t")));
        assert_eq!(
            client.auth_url("/tokens"),
            "https://api.ionos.com/auth/v1/tokens"
        );
    }

    #[test]
    fn append_query_encodes() {
        let url = append_query(
            "http://x/y",
            &[("depth", "1".into()), ("offset", "0".into())],
        );
        assert_eq!(url, "http://x/y?depth=1&offset=0");
    }

    #[test]
    fn api_error_parses_messages() {
        let body = r#"{"httpStatus":422,"messages":[{"errorCode":"VDC-ERR","message":"invalid location"}]}"#;
        let err = api_error(422, body);
        match err {
            IonosError::Api {
                status,
                codes,
                message,
            } => {
                assert_eq!(status, 422);
                assert_eq!(codes, vec!["VDC-ERR".to_string()]);
                assert_eq!(message, "invalid location");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
