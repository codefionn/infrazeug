//! Authentication for the NetBird Management API.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use std::fmt;

/// A credential accepted by the NetBird Management API.
#[derive(Clone)]
pub enum Auth {
    /// A personal access token sent as `Authorization: Token <token>`.
    PersonalAccessToken(String),
    /// An OAuth access token sent as `Authorization: Bearer <token>`.
    OAuthToken(String),
}

impl Auth {
    /// Create personal-access-token authentication.
    pub fn personal_access_token(token: impl Into<String>) -> Self {
        Self::PersonalAccessToken(token.into())
    }

    /// Create OAuth bearer-token authentication.
    pub fn oauth_token(token: impl Into<String>) -> Self {
        Self::OAuthToken(token.into())
    }

    pub(crate) fn apply(&self, headers: &mut HeaderMap) -> Result<(), String> {
        let value = match self {
            Self::PersonalAccessToken(token) => format!("Token {token}"),
            Self::OAuthToken(token) => format!("Bearer {token}"),
        };
        let value = HeaderValue::from_str(&value).map_err(|error| error.to_string())?;
        headers.insert(AUTHORIZATION, value);
        Ok(())
    }
}

impl fmt::Debug for Auth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PersonalAccessToken(_) => {
                formatter.write_str("Auth::PersonalAccessToken([redacted])")
            }
            Self::OAuthToken(_) => formatter.write_str("Auth::OAuthToken([redacted])"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_both_supported_authorization_schemes() {
        let mut headers = HeaderMap::new();
        Auth::personal_access_token("pat")
            .apply(&mut headers)
            .unwrap();
        assert_eq!(headers[AUTHORIZATION], "Token pat");

        Auth::oauth_token("oauth").apply(&mut headers).unwrap();
        assert_eq!(headers[AUTHORIZATION], "Bearer oauth");
    }
}
