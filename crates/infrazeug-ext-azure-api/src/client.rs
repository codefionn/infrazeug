use crate::auth::{AzureAuth, AzureCredentials};
use crate::error::{AzureError, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;

pub const ARM_API_VERSION: &str = "2023-09-01";

#[derive(Clone)]
pub struct AzureConfig {
    pub auth: AzureAuth,
}

impl AzureConfig {
    pub fn new(creds: AzureCredentials) -> Self {
        Self {
            auth: AzureAuth::new(creds),
        }
    }
}

#[derive(Clone)]
pub struct AzureClient {
    http: Client,
    config: AzureConfig,
}

impl AzureClient {
    pub fn new(config: AzureConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    pub fn subscription_id(&self) -> &str {
        self.config.auth.subscription_id()
    }

    pub(crate) async fn arm_get<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let resp = self.arm_request(Method::GET, url, None::<&()>).await?;
        let (status, body) = consume(resp).await?;
        decode(status, &body)
    }

    pub(crate) async fn arm_put<B: Serialize, T: DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        let resp = self.arm_request(Method::PUT, url, Some(body)).await?;
        let (status, text) = consume(resp).await?;
        decode(status, &text)
    }

    pub(crate) async fn arm_post<B: Serialize, T: DeserializeOwned>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T> {
        let resp = self.arm_request(Method::POST, url, Some(body)).await?;
        let (status, text) = consume(resp).await?;
        decode(status, &text)
    }

    pub(crate) async fn storage_request(
        &self,
        method: Method,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&[u8]>,
    ) -> Result<(StatusCode, String)> {
        let token = self.config.auth.storage_token().await?;
        let mut header_map = HeaderMap::new();
        header_map.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|e| AzureError::Auth(e.to_string()))?,
        );
        for (k, v) in headers {
            header_map.insert(
                reqwest::header::HeaderName::from_bytes(k.as_bytes())
                    .map_err(|e| AzureError::Auth(e.to_string()))?,
                HeaderValue::from_str(v).map_err(|e| AzureError::Auth(e.to_string()))?,
            );
        }
        let mut req = self.http.request(method, url).headers(header_map);
        if let Some(body) = body {
            req = req.body(body.to_vec());
        }
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Ok((status, text))
    }

    async fn arm_request<B: Serialize>(
        &self,
        method: Method,
        url: &str,
        body: Option<&B>,
    ) -> Result<reqwest::Response> {
        let token = self.config.auth.management_token().await?;
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|e| AzureError::Auth(e.to_string()))?,
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
    Ok((resp.status(), resp.text().await?))
}

fn decode<T: DeserializeOwned>(status: StatusCode, body: &str) -> Result<T> {
    if status.is_success() {
        serde_json::from_str(body).map_err(AzureError::from)
    } else {
        Err(AzureError::Api {
            status: status.as_u16(),
            message: body.to_string(),
        })
    }
}

pub(crate) fn api_error(status: StatusCode, body: &str) -> AzureError {
    AzureError::Api {
        status: status.as_u16(),
        message: body.to_string(),
    }
}
