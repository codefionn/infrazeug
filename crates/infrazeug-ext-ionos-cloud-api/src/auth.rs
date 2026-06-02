//! IONOS Cloud API authentication.
//!
//! The Cloud API accepts either a Bearer token (JWT generated via the Auth API
//! or the DCD Token Manager) or HTTP Basic credentials (username/password, only
//! when 2-Factor Authentication is not enabled). Users with multiple contracts
//! may need to send `X-Contract-Number` on each request.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::header::HeaderValue;

/// Authentication material for IONOS Cloud API calls.
#[derive(Clone)]
pub enum Auth {
    /// `Authorization: Bearer <token>`
    Token(String),
    /// `Authorization: Basic base64(username:password)`
    Basic {
        /// IONOS Cloud account username (email).
        username: String,
        /// IONOS Cloud account password.
        password: String,
    },
}

impl Auth {
    /// Bearer token authentication.
    pub fn token(token: impl Into<String>) -> Self {
        Self::Token(token.into())
    }

    /// HTTP Basic authentication.
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Build the `Authorization` header value for one request.
    pub(crate) fn authorization_header(&self) -> Result<HeaderValue, crate::error::IonosError> {
        let value = match self {
            Self::Token(token) => format!("Bearer {token}"),
            Self::Basic { username, password } => {
                let encoded = STANDARD.encode(format!("{username}:{password}"));
                format!("Basic {encoded}")
            }
        };
        HeaderValue::from_str(&value).map_err(|e| crate::error::IonosError::Auth(e.to_string()))
    }
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token(_) => f.debug_tuple("Token").field(&"<redacted>").finish(),
            Self::Basic { username, .. } => f
                .debug_struct("Basic")
                .field("username", username)
                .field("password", &"<redacted>")
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_header() {
        let auth = Auth::token("jwt-here");
        let header = auth.authorization_header().unwrap();
        assert_eq!(header.to_str().unwrap(), "Bearer jwt-here");
    }

    #[test]
    fn basic_header() {
        let auth = Auth::basic("user@example.com", "secret");
        let header = auth.authorization_header().unwrap();
        assert_eq!(
            header.to_str().unwrap(),
            "Basic dXNlckBleGFtcGxlLmNvbTpzZWNyZXQ="
        );
    }

    #[test]
    fn debug_redacts_secrets() {
        let rendered = format!("{:?}", Auth::token("super-secret"));
        assert!(!rendered.contains("super-secret"));
        let rendered = format!("{:?}", Auth::basic("user@example.com", "pw"));
        assert!(rendered.contains("user@example.com"));
        assert!(!rendered.contains("pw"));
    }
}
