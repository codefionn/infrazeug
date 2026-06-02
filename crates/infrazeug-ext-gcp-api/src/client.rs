//! Bearer-authenticated HTTP client for GCP REST APIs.

use crate::auth::GcpAuth;
use crate::error::{GcpError, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Connection configuration for a GCP API client.
#[derive(Clone)]
pub struct GcpConfig {
    pub auth: GcpAuth,
}

impl GcpConfig {
    pub fn new(auth: GcpAuth) -> Self {
        Self { auth }
    }
}

/// An authenticated GCP API client.
#[derive(Clone)]
pub struct GcpClient {
    http: Client,
    config: GcpConfig,
}

impl GcpClient {
    pub fn new(config: GcpConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    pub fn config(&self) -> &GcpConfig {
        &self.config
    }

    pub fn project_id(&self) -> &str {
        self.config.auth.project_id()
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let resp = self.request(Method::GET, url, None::<&()>).await?;
        let (status, body) = consume(resp).await?;
        decode(status, &body)
    }

    pub(crate) async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        let resp = self.request(Method::POST, url, Some(body)).await?;
        let (status, text) = consume(resp).await?;
        decode(status, &text)
    }

    async fn request<B: Serialize>(
        &self,
        method: Method,
        url: &str,
        body: Option<&B>,
    ) -> Result<reqwest::Response> {
        let token = self.config.auth.access_token().await?;
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|e| GcpError::Auth(e.to_string()))?,
        );
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
        serde_json::from_str(body).map_err(GcpError::from)
    } else {
        Err(GcpError::Api {
            status: status.as_u16(),
            message: body.to_string(),
        })
    }
}

pub(crate) fn api_error(status: StatusCode, body: &str) -> GcpError {
    GcpError::Api {
        status: status.as_u16(),
        message: body.to_string(),
    }
}
