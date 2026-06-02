//! Account and token verification endpoints.

use crate::client::CloudflareClient;
use crate::error::Result;
use crate::types::ListQuery;
use serde::Deserialize;

/// Token verification status from `GET /user/tokens/verify`.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenVerify {
    pub id: Option<String>,
    pub status: String,
}

/// Cloudflare account profile.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct User {
    pub id: Option<String>,
    pub email: Option<String>,
    pub username: Option<String>,
}

impl CloudflareClient {
    /// `GET /user/tokens/verify` — check that the configured token is valid.
    pub async fn verify_token(&self) -> Result<TokenVerify> {
        let (value, _) = self
            .get("/user/tokens/verify", &ListQuery::default())
            .await?;
        Ok(value)
    }

    /// `GET /user` — fetch the authenticated account profile.
    pub async fn user(&self) -> Result<User> {
        let (value, _) = self.get("/user", &ListQuery::default()).await?;
        Ok(value)
    }
}
