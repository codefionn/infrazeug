//! Cloudflare API authentication.
//!
//! Prefer scoped [API tokens](Auth::token) (`Authorization: Bearer …`). The legacy
//! global API key plus account email is still supported for older automation.

use reqwest::header::{HeaderMap, HeaderValue};

/// Authentication material for Cloudflare API calls.
#[derive(Clone)]
pub enum Auth {
    /// `Authorization: Bearer <api_token>`
    Token(String),
    /// `X-Auth-Email` + `X-Auth-Key` (global API key).
    GlobalKey {
        /// Account email address.
        email: String,
        /// Global API key from the dashboard.
        api_key: String,
    },
}

impl Auth {
    /// Bearer token authentication (recommended).
    pub fn token(token: impl Into<String>) -> Self {
        Self::Token(token.into())
    }

    /// Global API key authentication.
    pub fn global_key(email: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self::GlobalKey {
            email: email.into(),
            api_key: api_key.into(),
        }
    }

    /// Apply auth headers to a request.
    pub(crate) fn apply(
        &self,
        headers: &mut HeaderMap,
    ) -> Result<(), crate::error::CloudflareError> {
        match self {
            Self::Token(token) => {
                let value = format!("Bearer {token}");
                headers.insert(
                    reqwest::header::AUTHORIZATION,
                    HeaderValue::from_str(&value)
                        .map_err(|e| crate::error::CloudflareError::Auth(e.to_string()))?,
                );
            }
            Self::GlobalKey { email, api_key } => {
                headers.insert(
                    "X-Auth-Email",
                    HeaderValue::from_str(email)
                        .map_err(|e| crate::error::CloudflareError::Auth(e.to_string()))?,
                );
                headers.insert(
                    "X-Auth-Key",
                    HeaderValue::from_str(api_key)
                        .map_err(|e| crate::error::CloudflareError::Auth(e.to_string()))?,
                );
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token(_) => f.debug_tuple("Token").field(&"<redacted>").finish(),
            Self::GlobalKey { email, .. } => f
                .debug_struct("GlobalKey")
                .field("email", email)
                .field("api_key", &"<redacted>")
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_secrets() {
        let rendered = format!("{:?}", Auth::token("cfut_secret"));
        assert!(!rendered.contains("cfut_secret"));
        let rendered = format!("{:?}", Auth::global_key("user@example.com", "s3cret"));
        assert!(rendered.contains("user@example.com"));
        assert!(!rendered.contains("s3cret"));
    }
}
