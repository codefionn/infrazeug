//! GCP service-account authentication.

use crate::error::{GcpError, Result};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const DEFAULT_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";

/// Parsed service-account key material.
#[derive(Clone, Deserialize)]
pub struct ServiceAccountKey {
    pub project_id: String,
    pub client_email: String,
    private_key: String,
    #[serde(default)]
    token_uri: Option<String>,
}

impl std::fmt::Debug for ServiceAccountKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceAccountKey")
            .field("project_id", &self.project_id)
            .field("client_email", &self.client_email)
            .field("private_key", &"***")
            .field("token_uri", &self.token_uri)
            .finish()
    }
}

impl ServiceAccountKey {
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(GcpError::from)
    }
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: String,
    exp: u64,
    iat: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

/// Cached OAuth2 bearer token for service-account calls.
#[derive(Clone)]
pub struct GcpAuth {
    key: ServiceAccountKey,
    http: reqwest::Client,
    cache: std::sync::Arc<RwLock<Option<CachedToken>>>,
}

#[derive(Clone)]
struct CachedToken {
    access_token: String,
    expires_at: u64,
}

impl GcpAuth {
    pub fn new(key: ServiceAccountKey) -> Self {
        Self {
            key,
            // Bounded timeouts: without them a stalled peer hangs the apply run
            // forever (reqwest has no default request timeout).
            http: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(10))
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            cache: std::sync::Arc::new(RwLock::new(None)),
        }
    }

    pub fn project_id(&self) -> &str {
        &self.key.project_id
    }

    pub async fn access_token(&self) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Some(cached) = self.cache.read().await.clone() {
            if cached.expires_at > now + 60 {
                return Ok(cached.access_token);
            }
        }
        let token = self.fetch_token(now).await?;
        *self.cache.write().await = Some(token.clone());
        Ok(token.access_token)
    }

    async fn fetch_token(&self, now: u64) -> Result<CachedToken> {
        let aud = self
            .key
            .token_uri
            .clone()
            .unwrap_or_else(|| TOKEN_URL.into());
        // The endpoint comes from the key document: require https so a
        // tampered key JSON cannot redirect the signed (bearer-equivalent)
        // JWT assertion to an attacker over cleartext.
        if !aud.starts_with("https://") {
            return Err(GcpError::Auth(format!("token_uri must be https: {aud}")));
        }
        let claims = Claims {
            iss: &self.key.client_email,
            scope: DEFAULT_SCOPE,
            aud: aud.clone(),
            exp: now + 3600,
            iat: now,
        };
        let header = Header::new(Algorithm::RS256);
        let key = EncodingKey::from_rsa_pem(self.key.private_key.as_bytes())
            .map_err(|e| GcpError::Auth(e.to_string()))?;
        let assertion =
            encode(&header, &claims, &key).map_err(|e| GcpError::Auth(e.to_string()))?;

        let resp = self
            .http
            .post(&aud)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &assertion),
            ])
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(GcpError::Api {
                status: status.as_u16(),
                message: body,
            });
        }
        let parsed: TokenResponse = serde_json::from_str(&body)?;
        Ok(CachedToken {
            access_token: parsed.access_token,
            expires_at: now + parsed.expires_in.unwrap_or(3600),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_secrets() {
        let key = ServiceAccountKey::from_json(
            r#"{
                "project_id": "my-project",
                "client_email": "sa@my-project.iam.gserviceaccount.com",
                "private_key": "-----BEGIN RSA PRIVATE KEY-----\nSUPER-SECRET\n-----END RSA PRIVATE KEY-----",
                "token_uri": "https://oauth2.googleapis.com/token"
            }"#,
        )
        .unwrap();
        let rendered = format!("{key:?}");
        assert!(rendered.contains("my-project"));
        assert!(rendered.contains("***"));
        assert!(!rendered.contains("SUPER-SECRET"));
    }
}
