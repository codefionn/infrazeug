//! Authenticated OpenStack HTTP client (Keystone v3).

use crate::auth::{catalog_endpoint, OpenstackConfig, TokenResponse, TokenUser};
use crate::error::{OpenstackError, Result};
use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::{Client, Method};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

struct AuthState {
    token: String,
    user: TokenUser,
    catalog: Vec<crate::auth::CatalogEntry>,
    expires_at: DateTime<Utc>,
    username: String,
    password: String,
}

/// OpenStack client with cached Keystone token (password auth, project-scoped).
#[derive(Clone)]
pub struct OpenstackClient {
    http: Client,
    config: OpenstackConfig,
    auth: Arc<RwLock<Option<AuthState>>>,
}

impl OpenstackClient {
    /// Build an unauthenticated client (call [`authenticate`](Self::authenticate) next).
    pub fn new(config: OpenstackConfig) -> Self {
        Self {
            http: Client::new(),
            config,
            auth: Arc::new(RwLock::new(None)),
        }
    }

    /// Connection parameters (project id, region, auth URL).
    pub fn config(&self) -> &OpenstackConfig {
        &self.config
    }

    /// Authenticated Keystone user id (available after [`authenticate`](Self::authenticate)).
    pub async fn user_id(&self) -> Result<String> {
        Ok(self.auth_state().await?.user.id.clone())
    }

    /// Project id from the client configuration.
    pub fn project_id(&self) -> &str {
        &self.config.project_id
    }

    /// Authenticate with Keystone v3 password auth (project-scoped token).
    pub async fn authenticate(&self, username: &str, password: &str) -> Result<()> {
        let body = serde_json::json!({
            "auth": {
                "identity": {
                    "methods": ["password"],
                    "password": {
                        "user": {
                            "name": username,
                            "domain": { "name": self.config.domain },
                            "password": password
                        }
                    }
                },
                "scope": {
                    "project": {
                        "id": self.config.project_id,
                        "domain": { "name": self.config.domain }
                    }
                }
            }
        });
        let url = format!("{}/auth/tokens", self.config.auth_url.trim_end_matches('/'));
        let resp = self
            .http
            .post(&url)
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let token_hdr = resp
            .headers()
            .get("X-Subject-Token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body_text = resp.text().await?;
        if !status.is_success() {
            return Err(OpenstackError::Api {
                status: status.as_u16(),
                message: body_text,
            });
        }
        let token = token_hdr.ok_or_else(|| OpenstackError::Api {
            status: status.as_u16(),
            message: "Keystone response missing X-Subject-Token header".into(),
        })?;
        let parsed: TokenResponse = serde_json::from_str(&body_text)?;

        *self.auth.write().await = Some(AuthState {
            token,
            user: parsed.token.user,
            catalog: parsed.token.catalog,
            expires_at: parsed.token.expires_at,
            username: username.to_string(),
            password: password.to_string(),
        });
        Ok(())
    }

    async fn auth_state(&self) -> Result<AuthState> {
        loop {
            let guard = self.auth.read().await;
            let Some(state) = guard.as_ref() else {
                return Err(OpenstackError::NotAuthenticated);
            };
            let now = Utc::now();
            let needs_refresh = state.expires_at <= now + chrono::Duration::seconds(30);
            if !needs_refresh {
                return Ok(AuthState {
                    token: state.token.clone(),
                    user: state.user.clone(),
                    catalog: state.catalog.clone(),
                    expires_at: state.expires_at,
                    username: state.username.clone(),
                    password: state.password.clone(),
                });
            }
            let username = state.username.clone();
            let password = state.password.clone();
            drop(guard);
            self.authenticate(&username, &password).await?;
        }
    }

    fn identity_base(&self, catalog: &[crate::auth::CatalogEntry]) -> String {
        catalog_endpoint(catalog, "identity", &self.config.region)
            .unwrap_or_else(|| self.config.auth_url.trim_end_matches('/').to_string())
    }

    pub(crate) async fn identity_get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let state = self.auth_state().await?;
        let base = self.identity_base(&state.catalog);
        self.send_json(Method::GET, &base, path, None::<&()>, &state.token)
            .await
    }

    pub(crate) async fn identity_post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let state = self.auth_state().await?;
        let base = self.identity_base(&state.catalog);
        self.send_json(Method::POST, &base, path, Some(body), &state.token)
            .await
    }

    async fn send_json<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        base: &str,
        path: &str,
        body: Option<&B>,
        token: &str,
    ) -> Result<T> {
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        let url = format!("{}{}", base.trim_end_matches('/'), path);
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            "X-Subject-Token",
            HeaderValue::from_str(token).map_err(|e| OpenstackError::Url(e.to_string()))?,
        );
        let mut req = self.http.request(method.clone(), &url).headers(headers);
        if let Some(body) = body {
            req = req.header(CONTENT_TYPE, "application/json").json(body);
        }
        let resp = req.send().await?;
        decode(resp).await
    }
}

async fn decode<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
    let status = resp.status();
    let body = resp.text().await?;
    if status.is_success() {
        Ok(serde_json::from_str(&body)?)
    } else {
        Err(OpenstackError::Api {
            status: status.as_u16(),
            message: body,
        })
    }
}
