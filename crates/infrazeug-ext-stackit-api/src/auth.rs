//! STACKIT service-account authentication (token and key flows).

use crate::error::{Result, StackitError};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

const DEFAULT_TOKEN_URL: &str = "https://service-account.api.stackit.cloud/token";

/// Authentication material for STACKIT API calls.
#[derive(Clone)]
pub enum Auth {
    /// `Authorization: Bearer <service-account-token>` (deprecated by STACKIT but
    /// still supported for existing integrations).
    Token(String),
    /// Key flow: sign a short-lived JWT with the service-account private key and
    /// exchange it for a bearer access token.
    ServiceAccountKey {
        key: Arc<ServiceAccountKey>,
        /// PEM-encoded RSA private key. When omitted, the key embedded in
        /// [`ServiceAccountKey`] is used.
        private_key: Option<String>,
        provider: Arc<KeyFlowProvider>,
    },
}

impl Auth {
    /// Bearer token authentication (`STACKIT_SERVICE_ACCOUNT_TOKEN`).
    pub fn token(token: impl Into<String>) -> Self {
        Self::Token(token.into())
    }

    /// Key-flow authentication from a parsed service-account key JSON document.
    pub fn service_account_key(key: ServiceAccountKey, private_key: Option<String>) -> Self {
        Self::ServiceAccountKey {
            key: Arc::new(key),
            private_key,
            provider: Arc::new(KeyFlowProvider::new()),
        }
    }

    /// Parse a service-account key JSON document and configure key-flow auth.
    pub fn service_account_key_json(json: &str, private_key: Option<String>) -> Result<Self> {
        let key = ServiceAccountKey::from_json(json)?;
        Ok(Self::service_account_key(key, private_key))
    }

    pub(crate) async fn authorization_header(&self) -> Result<HeaderValue> {
        let value = match self {
            Self::Token(token) => format!("Bearer {token}"),
            Self::ServiceAccountKey {
                key,
                private_key,
                provider,
            } => {
                let access = provider.access_token(key, private_key.as_deref()).await?;
                format!("Bearer {access}")
            }
        };
        HeaderValue::from_str(&value).map_err(|e| StackitError::Auth(e.to_string()))
    }
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token(_) => f.debug_tuple("Token").field(&"<redacted>").finish(),
            Self::ServiceAccountKey { key, .. } => f
                .debug_struct("ServiceAccountKey")
                .field("key_id", &key.id)
                .field("private_key", &"<redacted>")
                .finish(),
        }
    }
}

/// Service-account key returned by the STACKIT portal or API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceAccountKey {
    pub id: String,
    pub credentials: ServiceAccountKeyCredentials,
    #[serde(default)]
    pub active: bool,
}

/// JWT signing metadata embedded in a service-account key.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceAccountKeyCredentials {
    pub aud: String,
    pub iss: String,
    pub kid: String,
    pub sub: String,
    #[serde(default)]
    pub private_key: Option<String>,
    #[serde(default)]
    pub token_endpoint: Option<String>,
}

impl ServiceAccountKey {
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(StackitError::from)
    }

    fn token_url(&self) -> String {
        self.credentials
            .token_endpoint
            .clone()
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| DEFAULT_TOKEN_URL.into())
    }

    fn private_key_pem<'a>(&'a self, override_key: Option<&'a str>) -> Result<&'a str> {
        if let Some(key) = override_key {
            return Ok(key);
        }
        self.credentials
            .private_key
            .as_deref()
            .ok_or_else(|| StackitError::Auth("service account private key is not set".into()))
    }
}

#[derive(Serialize)]
struct JwtClaims<'a> {
    iss: &'a str,
    sub: &'a str,
    jti: String,
    aud: &'a str,
    iat: u64,
    exp: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Clone, Debug)]
struct CachedToken {
    access_token: String,
    expires_at: u64,
}

/// Exchanges signed JWTs for short-lived bearer tokens (key flow).
#[derive(Debug)]
pub struct KeyFlowProvider {
    http: reqwest::Client,
    cache: Arc<RwLock<Option<CachedToken>>>,
}

impl KeyFlowProvider {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    async fn access_token(
        &self,
        key: &ServiceAccountKey,
        private_key: Option<&str>,
    ) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Some(cached) = self.cache.read().await.clone() {
            if cached.expires_at > now + 60 {
                return Ok(cached.access_token);
            }
        }
        let token = self.fetch_token(key, private_key, now).await?;
        *self.cache.write().await = Some(token.clone());
        Ok(token.access_token)
    }

    async fn fetch_token(
        &self,
        key: &ServiceAccountKey,
        private_key: Option<&str>,
        now: u64,
    ) -> Result<CachedToken> {
        let pem = key.private_key_pem(private_key)?;
        let claims = JwtClaims {
            iss: &key.credentials.iss,
            sub: &key.credentials.sub,
            jti: uuid::Uuid::new_v4().to_string(),
            aud: &key.credentials.aud,
            iat: now,
            exp: now + 3600,
        };
        let mut header = Header::new(Algorithm::RS512);
        header.kid = Some(key.credentials.kid.clone());
        let encoding_key = EncodingKey::from_rsa_pem(pem.as_bytes())
            .map_err(|e| StackitError::Auth(e.to_string()))?;
        let assertion = encode(&header, &claims, &encoding_key)
            .map_err(|e| StackitError::Auth(e.to_string()))?;

        let resp = self
            .http
            .post(key.token_url())
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!(
                "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={}",
                urlencoding::encode(&assertion)
            ))
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(StackitError::Api {
                status: status.as_u16(),
                message: body,
            });
        }
        let parsed: TokenResponse = serde_json::from_str(&body)?;
        let expires_in = parsed.expires_in.unwrap_or(3600);
        Ok(CachedToken {
            access_token: parsed.access_token,
            expires_at: now + expires_in,
        })
    }
}

impl Default for KeyFlowProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_token() {
        let rendered = format!("{:?}", Auth::token("super-secret"));
        assert!(!rendered.contains("super-secret"));
    }

    #[test]
    fn parses_service_account_key() {
        let json = r#"{
            "id": "key-id",
            "credentials": {
                "aud": "aud",
                "iss": "sa@example.com",
                "kid": "kid",
                "sub": "sub-id",
                "tokenEndpoint": "https://token.example/token"
            }
        }"#;
        let key = ServiceAccountKey::from_json(json).unwrap();
        assert_eq!(key.token_url(), "https://token.example/token");
    }
}
