//! Auth API token management (`/auth/v1/tokens`).

use crate::client::IonosClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// A token entry returned by the Auth API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<String>,
}

/// Response from `GET /tokens`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenList {
    pub tokens: Vec<TokenInfo>,
}

/// Response from `GET /tokens/generate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenGenerateResponse {
    pub token: String,
}

/// Response from token deletion endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDeleteResponse {
    pub success: bool,
}

/// Criteria for bulk token deletion.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TokenDeleteCriteria {
    /// Delete all tokens for the contract.
    All,
    /// Delete expired tokens.
    Expired,
    /// Delete the token used for the request.
    Current,
}

impl IonosClient {
    /// `GET /auth/v1/tokens/generate` — create a new API token.
    ///
    /// Requires Basic or Bearer authentication. Users with multiple contracts
    /// must set [`crate::IonosConfig::contract_number`].
    pub async fn generate_token(&self, ttl_seconds: Option<u32>) -> Result<String> {
        let mut query = Vec::new();
        if let Some(ttl) = ttl_seconds {
            query.push(("ttl", ttl.to_string()));
        }
        let resp: TokenGenerateResponse = self.get_auth("/tokens/generate", &query).await?;
        Ok(resp.token)
    }

    /// `GET /auth/v1/tokens` — list tokens created by the authenticated user.
    pub async fn list_tokens(&self) -> Result<Vec<TokenInfo>> {
        let resp: TokenList = self.get_auth("/tokens", &[]).await?;
        Ok(resp.tokens)
    }

    /// `GET /auth/v1/tokens/{tokenId}` — retrieve one token by key ID.
    pub async fn token_info(&self, token_id: &str) -> Result<TokenInfo> {
        self.get_auth(&format!("/tokens/{}", self.encode_path(token_id)), &[])
            .await
    }

    /// `DELETE /auth/v1/tokens?criteria=…` — delete tokens by criteria.
    pub async fn delete_tokens_by_criteria(&self, criteria: TokenDeleteCriteria) -> Result<bool> {
        let criteria = serde_json::to_value(criteria)?;
        let criteria = criteria.as_str().unwrap_or("ALL").to_string();
        let query = vec![("criteria", criteria)];
        let resp = self
            .send_auth(reqwest::Method::DELETE, "/tokens", &query, None)
            .await?;
        let (status, body) = super::client::consume(resp).await?;
        if status.is_success() {
            let parsed: TokenDeleteResponse = serde_json::from_str(&body)?;
            Ok(parsed.success)
        } else {
            Err(super::client::api_error(status.as_u16(), &body))
        }
    }

    /// `DELETE /auth/v1/tokens/{tokenId}` — delete one token by key ID.
    pub async fn delete_token(&self, token_id: &str) -> Result<bool> {
        let resp = self
            .send_auth(
                reqwest::Method::DELETE,
                &format!("/tokens/{}", self.encode_path(token_id)),
                &[],
                None,
            )
            .await?;
        let (status, body) = super::client::consume(resp).await?;
        if status.is_success() {
            let parsed: TokenDeleteResponse = serde_json::from_str(&body)?;
            Ok(parsed.success)
        } else {
            Err(super::client::api_error(status.as_u16(), &body))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn criteria_serializes_uppercase() {
        let json = serde_json::to_string(&TokenDeleteCriteria::Expired).unwrap();
        assert_eq!(json, "\"EXPIRED\"");
    }
}
