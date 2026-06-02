//! HTTP client for the Cloudflare API v4.

use crate::auth::Auth;
use crate::error::{CloudflareError, Result};
use crate::types::{ApiErrorEntry, CloudflareResponse, ListQuery};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Default production API host.
pub const DEFAULT_HOST: &str = "https://api.cloudflare.com";
pub(crate) const API_PATH: &str = "/client/v4";

/// Connection configuration for a Cloudflare API client.
#[derive(Debug, Clone)]
pub struct CloudflareConfig {
    /// API host (default `https://api.cloudflare.com`). Trailing slashes are stripped.
    pub host: String,
    /// Authentication method (API token or global API key).
    pub auth: Auth,
    /// Default account id for account-scoped APIs (R2, KV, …).
    pub account_id: Option<String>,
}

impl CloudflareConfig {
    /// Create a configuration with the default production host.
    pub fn new(auth: Auth) -> Self {
        Self {
            host: DEFAULT_HOST.into(),
            auth,
            account_id: None,
        }
    }

    /// Override the API host.
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into().trim_end_matches('/').to_string();
        self
    }

    /// Set the default Cloudflare account id (`CLOUDFLARE_ACCOUNT_ID`).
    pub fn with_account_id(mut self, account_id: impl Into<String>) -> Self {
        self.account_id = Some(account_id.into());
        self
    }
}

/// An authenticated Cloudflare API client.
///
/// ```no_run
/// use infrazeug_ext_cloudflare_api::{Auth, CloudflareClient, CloudflareConfig};
///
/// # async fn run() -> infrazeug_ext_cloudflare_api::Result<()> {
/// let client = CloudflareClient::new(CloudflareConfig::new(Auth::token(
///     std::env::var("CLOUDFLARE_API_TOKEN").unwrap(),
/// )));
///
/// let verified = client.verify_token().await?;
/// println!("token status: {}", verified.status);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CloudflareClient {
    http: Client,
    config: CloudflareConfig,
}

impl CloudflareClient {
    /// Build a client with the given configuration.
    pub fn new(config: CloudflareConfig) -> Self {
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
    pub fn config(&self) -> &CloudflareConfig {
        &self.config
    }

    pub(crate) fn api_url(&self, path: &str) -> String {
        join_url(&self.config.host, API_PATH, path)
    }

    pub(crate) fn encode_path(&self, s: &str) -> String {
        urlencoding::encode(s).into_owned()
    }

    fn apply_auth(&self, headers: &mut HeaderMap) -> Result<()> {
        self.config.auth.apply(headers)
    }

    pub(crate) async fn send(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&str>,
    ) -> Result<reqwest::Response> {
        let url = append_query(&self.api_url(path), query);
        let mut headers = HeaderMap::new();
        self.apply_auth(&mut headers)?;
        if body.is_some() {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }

        let mut req = self.http.request(method, &url).headers(headers);
        if let Some(body) = body {
            req = req.body(body.to_string());
        }
        Ok(req.send().await?)
    }

    pub(crate) async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &ListQuery,
    ) -> Result<(T, Option<crate::types::ResultInfo>)> {
        let params = query.as_params();
        let resp = self.send(Method::GET, path, &params, None).await?;
        decode_envelope(resp).await
    }

    pub(crate) async fn get_all<T: DeserializeOwned>(
        &self,
        path: &str,
        mut query: ListQuery,
    ) -> Result<Vec<T>> {
        let per_page = query.per_page.unwrap_or(100);
        query.per_page = Some(per_page);
        let mut page = query.page.unwrap_or(1);
        let mut out = Vec::new();

        loop {
            query.page = Some(page);
            let (batch, info): (Vec<T>, _) = self.get(path, &query).await?;
            let empty = batch.is_empty();
            out.extend(batch);
            let more = info.as_ref().map(|i| i.has_more()).unwrap_or(false);
            if !more || empty {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    pub(crate) async fn get_all_with_params<T: DeserializeOwned>(
        &self,
        path: &str,
        base_params: Vec<(&str, String)>,
    ) -> Result<Vec<T>> {
        let per_page = base_params
            .iter()
            .find(|(k, _)| *k == "per_page")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(100u32);
        let mut page = 1u32;
        let mut out = Vec::new();

        loop {
            let mut params = base_params.clone();
            params.push(("page", page.to_string()));
            if !params.iter().any(|(k, _)| *k == "per_page") {
                params.push(("per_page", per_page.to_string()));
            }
            let resp = self.send(Method::GET, path, &params, None).await?;
            let (batch, info): (Vec<T>, _) = decode_envelope(resp).await?;
            let empty = batch.is_empty();
            out.extend(batch);
            let more = info.as_ref().map(|i| i.has_more()).unwrap_or(false);
            if !more || empty {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    pub(crate) async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let body = serde_json::to_string(body)?;
        let resp = self.send(Method::POST, path, &[], Some(&body)).await?;
        let (value, _) = decode_envelope(resp).await?;
        Ok(value)
    }

    pub(crate) async fn put_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let body = serde_json::to_string(body)?;
        let resp = self.send(Method::PUT, path, &[], Some(&body)).await?;
        let (value, _) = decode_envelope(resp).await?;
        Ok(value)
    }

    pub(crate) async fn patch_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let body = serde_json::to_string(body)?;
        let resp = self.send(Method::PATCH, path, &[], Some(&body)).await?;
        let (value, _) = decode_envelope(resp).await?;
        Ok(value)
    }

    pub(crate) async fn delete(&self, path: &str) -> Result<()> {
        let resp = self.send(Method::DELETE, path, &[], None).await?;
        let _: Option<serde_json::Value> = decode_envelope(resp).await?.0;
        Ok(())
    }

    pub(crate) async fn get_with_headers<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        headers: &[(&str, &str)],
    ) -> Result<T> {
        let resp = self
            .send_with_headers(Method::GET, path, query, None, headers)
            .await?;
        let (value, _) = decode_envelope(resp).await?;
        Ok(value)
    }

    pub(crate) async fn post_json_with_headers<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        headers: &[(&str, &str)],
    ) -> Result<T> {
        let body = serde_json::to_string(body)?;
        let resp = self
            .send_with_headers(Method::POST, path, &[], Some(&body), headers)
            .await?;
        let (value, _) = decode_envelope(resp).await?;
        Ok(value)
    }

    pub(crate) async fn patch_with_headers<T: DeserializeOwned>(
        &self,
        path: &str,
        headers: &[(&str, &str)],
    ) -> Result<T> {
        let resp = self
            .send_with_headers(Method::PATCH, path, &[], None, headers)
            .await?;
        let (value, _) = decode_envelope(resp).await?;
        Ok(value)
    }

    pub(crate) async fn send_with_headers(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&str>,
        extra_headers: &[(&str, &str)],
    ) -> Result<reqwest::Response> {
        let url = append_query(&self.api_url(path), query);
        let mut headers = HeaderMap::new();
        self.apply_auth(&mut headers)?;
        if body.is_some() {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }
        for (name, value) in extra_headers {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(value),
            ) {
                headers.insert(name, value);
            }
        }

        let mut req = self.http.request(method, &url).headers(headers);
        if let Some(body) = body {
            req = req.body(body.to_string());
        }
        Ok(req.send().await?)
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

pub(crate) async fn decode_envelope<T: DeserializeOwned>(
    resp: reqwest::Response,
) -> Result<(T, Option<crate::types::ResultInfo>)> {
    let status = resp.status();
    let body = resp.text().await?;
    decode_body(status, &body)
}

pub(crate) fn decode_body<T: DeserializeOwned>(
    status: StatusCode,
    body: &str,
) -> Result<(T, Option<crate::types::ResultInfo>)> {
    let envelope: CloudflareResponse<T> = serde_json::from_str(body)?;
    if status.is_success() && envelope.success {
        let value = envelope
            .result
            .ok_or_else(|| api_error(status.as_u16(), body))?;
        Ok((value, envelope.result_info))
    } else {
        Err(api_error_from_envelope(status.as_u16(), &envelope, body))
    }
}

pub(crate) fn api_error(status: u16, body: &str) -> CloudflareError {
    if let Ok(envelope) = serde_json::from_str::<CloudflareResponse<serde_json::Value>>(body) {
        return api_error_from_envelope(status, &envelope, body);
    }
    CloudflareError::Api {
        status,
        codes: Vec::new(),
        message: body.trim().to_string(),
    }
}

fn api_error_from_envelope<T>(
    status: u16,
    envelope: &CloudflareResponse<T>,
    body: &str,
) -> CloudflareError {
    let codes: Vec<u64> = envelope.errors.iter().filter_map(|e| e.code).collect();
    let message = format_api_errors(&envelope.errors)
        .or_else(|| format_api_errors(&envelope.messages))
        .unwrap_or_else(|| body.trim().to_string());
    CloudflareError::Api {
        status,
        codes,
        message,
    }
}

fn format_api_errors(errors: &[ApiErrorEntry]) -> Option<String> {
    let parts: Vec<String> = errors.iter().filter_map(|e| e.message.clone()).collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ApiErrorEntry;

    #[test]
    fn config_trims_host() {
        let cfg = CloudflareConfig::new(Auth::token("t")).with_host("https://api.cloudflare.com/");
        assert_eq!(cfg.host, "https://api.cloudflare.com");
    }

    #[test]
    fn api_url_joins() {
        let client = CloudflareClient::new(CloudflareConfig::new(Auth::token("t")));
        assert_eq!(
            client.api_url("/zones"),
            "https://api.cloudflare.com/client/v4/zones"
        );
    }

    #[test]
    fn decode_success_object() {
        let body = r#"{"success":true,"errors":[],"messages":[],"result":{"id":"abc"}}"#;
        let (value, _): (serde_json::Value, _) = decode_body(StatusCode::OK, body).unwrap();
        assert_eq!(value["id"], "abc");
    }

    #[test]
    fn decode_api_error() {
        let body =
            r#"{"success":false,"errors":[{"code":1003,"message":"invalid zone"}],"result":null}"#;
        let err = decode_body::<serde_json::Value>(StatusCode::BAD_REQUEST, body).unwrap_err();
        match err {
            CloudflareError::Api {
                status,
                codes,
                message,
            } => {
                assert_eq!(status, 400);
                assert_eq!(codes, vec![1003]);
                assert_eq!(message, "invalid zone");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn format_errors_joins() {
        let errors = vec![
            ApiErrorEntry {
                code: Some(1),
                message: Some("a".into()),
            },
            ApiErrorEntry {
                code: Some(2),
                message: Some("b".into()),
            },
        ];
        assert_eq!(format_api_errors(&errors).unwrap(), "a; b");
    }
}
