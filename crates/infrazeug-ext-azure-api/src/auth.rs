use crate::error::{AzureError, Result};
use serde::Deserialize;
use tokio::sync::RwLock;

const TOKEN_URL_TEMPLATE: &str = "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token";
const MANAGEMENT_SCOPE: &str = "https://management.azure.com/.default";

#[derive(Debug, Clone)]
pub struct AzureCredentials {
    pub tenant_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub subscription_id: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Clone)]
struct CachedToken {
    access_token: String,
    expires_at: u64,
}

#[derive(Clone)]
pub struct AzureAuth {
    creds: AzureCredentials,
    http: reqwest::Client,
    cache: std::sync::Arc<RwLock<Option<CachedToken>>>,
}

impl AzureAuth {
    pub fn new(creds: AzureCredentials) -> Self {
        Self {
            creds,
            http: reqwest::Client::new(),
            cache: std::sync::Arc::new(RwLock::new(None)),
        }
    }

    pub fn subscription_id(&self) -> &str {
        &self.creds.subscription_id
    }

    pub async fn management_token(&self) -> Result<String> {
        self.token(MANAGEMENT_SCOPE).await
    }

    pub async fn storage_token(&self) -> Result<String> {
        self.token("https://storage.azure.com/.default").await
    }

    async fn token(&self, scope: &str) -> Result<String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Some(cached) = self.cache.read().await.clone() {
            if cached.expires_at > now + 60 {
                return Ok(cached.access_token);
            }
        }
        let url = TOKEN_URL_TEMPLATE.replace("{tenant}", &self.creds.tenant_id);
        let resp = self
            .http
            .post(&url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", self.creds.client_id.as_str()),
                ("client_secret", self.creds.client_secret.as_str()),
                ("scope", scope),
            ])
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(AzureError::Api {
                status: status.as_u16(),
                message: body,
            });
        }
        let parsed: TokenResponse = serde_json::from_str(&body)?;
        let token = CachedToken {
            access_token: parsed.access_token,
            expires_at: now + parsed.expires_in.unwrap_or(3600),
        };
        *self.cache.write().await = Some(token.clone());
        Ok(token.access_token)
    }
}
