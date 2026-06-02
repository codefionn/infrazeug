//! HTTP client for the Backblaze B2 Native API.

use crate::auth::Credentials;
use crate::error::{BackblazeError, Result};
use crate::types::B2ErrorResponse;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Default authorize-account host (cluster-independent).
pub const DEFAULT_AUTHORIZE_HOST: &str = "https://api.backblazeb2.com";
/// Default B2 Native API version path segment.
pub const DEFAULT_API_VERSION: &str = "v4";

/// Connection configuration for a B2 Native API client.
#[derive(Debug, Clone)]
pub struct BackblazeConfig {
    /// Host used for [`b2_authorize_account`](Self::authorize) only.
    pub authorize_host: String,
    /// API version path segment (`v4` recommended).
    pub api_version: String,
    /// Application key credentials.
    pub credentials: Credentials,
}

impl BackblazeConfig {
    /// Create a configuration with the default authorize host and API version.
    pub fn new(credentials: Credentials) -> Self {
        Self {
            authorize_host: DEFAULT_AUTHORIZE_HOST.into(),
            api_version: DEFAULT_API_VERSION.into(),
            credentials,
        }
    }

    /// Override the authorize-account host (trailing slashes stripped).
    pub fn with_authorize_host(mut self, host: impl Into<String>) -> Self {
        self.authorize_host = host.into().trim_end_matches('/').to_string();
        self
    }

    /// Override the API version (`v3` or `v4`).
    pub fn with_api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = version.into();
        self
    }
}

#[derive(Debug, Clone)]
struct AuthSession {
    account_id: String,
    api_url: String,
    auth_token: String,
    download_url: Option<String>,
}

/// An authenticated Backblaze B2 Native API client.
///
/// The client lazily calls `b2_authorize_account` on first use and re-authorizes
/// when the auth token expires.
#[derive(Clone)]
pub struct BackblazeClient {
    http: Client,
    config: BackblazeConfig,
    session: Arc<RwLock<Option<AuthSession>>>,
}

impl BackblazeClient {
    /// Build a client with the given configuration.
    pub fn new(config: BackblazeConfig) -> Self {
        Self {
            http: Client::new(),
            config,
            session: Arc::new(RwLock::new(None)),
        }
    }

    /// Use a pre-configured [`reqwest::Client`] (custom timeouts, proxy, …).
    pub fn with_http_client(mut self, http: Client) -> Self {
        self.http = http;
        self
    }

    /// Return a clone of the client configuration.
    pub fn config(&self) -> &BackblazeConfig {
        &self.config
    }

    /// Account ID from the current authorization session.
    pub async fn account_id(&self) -> Result<String> {
        Ok(self.session().await?.account_id)
    }

    /// Download base URL from the current authorization session, when present.
    pub async fn download_url(&self) -> Result<Option<String>> {
        Ok(self.session().await?.download_url)
    }

    pub(crate) async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        operation: &str,
        body: &B,
    ) -> Result<T> {
        self.request_json(Method::POST, operation, Some(body)).await
    }

    pub(crate) async fn request_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        method: Method,
        operation: &str,
        body: Option<&B>,
    ) -> Result<T> {
        let mut retried = false;
        loop {
            let session = self.session().await?;
            let url = format!(
                "{}/b2api/{}/{}",
                session.api_url.trim_end_matches('/'),
                self.config.api_version,
                operation
            );
            let mut headers = HeaderMap::new();
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&session.auth_token)
                    .map_err(|e| BackblazeError::Auth(e.to_string()))?,
            );
            let body_str = if let Some(body) = body {
                headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                Some(serde_json::to_string(body)?)
            } else {
                None
            };
            let mut req = self.http.request(method.clone(), &url).headers(headers);
            if let Some(body) = &body_str {
                req = req.body(body.clone());
            }
            let resp = req.send().await?;
            match decode_response(resp).await {
                Ok(value) => return Ok(value),
                Err(err) if !retried && err.is_auth_token_error() => {
                    self.clear_session().await;
                    retried = true;
                    continue;
                }
                Err(err) => return Err(err),
            }
        }
    }

    async fn session(&self) -> Result<AuthSession> {
        {
            let guard = self.session.read().await;
            if let Some(session) = guard.as_ref() {
                return Ok(session.clone());
            }
        }
        self.authorize().await?;
        self.session
            .read()
            .await
            .clone()
            .ok_or_else(|| BackblazeError::Auth("authorization failed".into()))
    }

    async fn clear_session(&self) {
        *self.session.write().await = None;
    }

    async fn authorize(&self) -> Result<()> {
        let url = format!(
            "{}/b2api/{}/b2_authorize_account",
            self.config.authorize_host.trim_end_matches('/'),
            self.config.api_version
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&self.config.credentials.basic_auth_header())
                .map_err(|e| BackblazeError::Auth(e.to_string()))?,
        );
        let resp = self.http.get(&url).headers(headers).send().await?;
        let session: AuthorizeResponse = decode_response(resp).await?;
        let api_url = session.api_info.storage_api.api_url.ok_or_else(|| {
            BackblazeError::Auth("authorize response missing storage apiUrl".into())
        })?;
        *self.session.write().await = Some(AuthSession {
            account_id: session.account_id,
            api_url,
            auth_token: session.authorization_token,
            download_url: session.api_info.storage_api.download_url,
        });
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct AuthorizeResponse {
    #[serde(rename = "accountId")]
    account_id: String,
    #[serde(rename = "authorizationToken")]
    authorization_token: String,
    #[serde(rename = "apiInfo")]
    api_info: ApiInfo,
}

#[derive(Debug, Deserialize)]
struct ApiInfo {
    #[serde(rename = "storageApi")]
    storage_api: StorageApiInfo,
}

#[derive(Debug, Deserialize)]
struct StorageApiInfo {
    #[serde(rename = "apiUrl")]
    api_url: Option<String>,
    #[serde(rename = "downloadUrl")]
    download_url: Option<String>,
}

async fn decode_response<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
    let status = resp.status();
    let body = resp.text().await?;
    decode_body(status, &body)
}

pub(crate) fn decode_body<T: DeserializeOwned>(status: StatusCode, body: &str) -> Result<T> {
    if status.is_success() {
        return serde_json::from_str(body).map_err(BackblazeError::from);
    }
    if let Ok(err) = serde_json::from_str::<B2ErrorResponse>(body) {
        return Err(BackblazeError::Api {
            status: err.status,
            code: err.code,
            message: err.message,
        });
    }
    Err(BackblazeError::Api {
        status: status.as_u16(),
        code: "http_error".into(),
        message: body.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Credentials;

    #[test]
    fn config_defaults() {
        let cfg = BackblazeConfig::new(Credentials::new("id", "key"));
        assert_eq!(cfg.authorize_host, DEFAULT_AUTHORIZE_HOST);
        assert_eq!(cfg.api_version, DEFAULT_API_VERSION);
    }

    #[test]
    fn decode_success_bucket() {
        let body = r#"{"bucketId":"abc","bucketName":"logs","bucketType":"allPrivate"}"#;
        let value: serde_json::Value = decode_body(StatusCode::OK, body).unwrap();
        assert_eq!(value["bucketId"], "abc");
    }

    #[test]
    fn decode_api_error() {
        let body = r#"{"status":401,"code":"expired_auth_token","message":"token expired"}"#;
        let err = decode_body::<serde_json::Value>(StatusCode::UNAUTHORIZED, body).unwrap_err();
        match err {
            BackblazeError::Api {
                status,
                code,
                message,
            } => {
                assert_eq!(status, 401);
                assert_eq!(code, "expired_auth_token");
                assert_eq!(message, "token expired");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn auth_token_error_detection() {
        let err = BackblazeError::Api {
            status: 401,
            code: "bad_auth_token".into(),
            message: "bad".into(),
        };
        assert!(err.is_auth_token_error());
    }
}
