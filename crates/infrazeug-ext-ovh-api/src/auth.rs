//! OVHcloud request authentication.
//!
//! Two authentication methods are supported:
//!
//! ## Classic (Application Key / Consumer Key)
//!
//! Every authenticated OVH call carries four headers:
//!
//! - `X-Ovh-Application`: the application key
//! - `X-Ovh-Consumer`: the consumer key
//! - `X-Ovh-Timestamp`: the request time, in sync with the API server
//! - `X-Ovh-Signature`: `"$1$" + SHA1_HEX(secret + "+" + consumerKey + "+" +
//!   method + "+" + url + "+" + body + "+" + timestamp)`
//!
//! ## OAuth2 (Service Account)
//!
//! Uses a `client_id` / `client_secret` pair to obtain a short-lived Bearer
//! token via the OAuth2 client-credentials flow (`POST /auth/oauth2/token`).
//! Requests are authenticated with `Authorization: Bearer <access_token>`.
//!
//! The signing function is pure and unit-tested against SHA-1 vectors computed
//! independently of this code, so a regression in the canonical string is
//! caught without a live API.

use serde::Deserialize;
use sha1::{Digest, Sha1};

/// OVH API application + consumer credentials (classic auth).
///
/// Obtain `application_key`/`application_secret` by creating an application at
/// <https://eu.api.ovh.com/createApp/>, and a `consumer_key` by validating a
/// credential request (<https://eu.api.ovh.com/createToken/>). The consumer key
/// is optional: application-only clients can still reach unauthenticated routes
/// such as `/auth/time`.
#[derive(Clone)]
pub struct Credentials {
    /// Application key (`X-Ovh-Application`).
    pub application_key: String,
    /// Application secret, used only to compute signatures — never sent.
    pub application_secret: String,
    /// Consumer key (`X-Ovh-Consumer`), tying the call to a validated token.
    pub consumer_key: Option<String>,
}

impl Credentials {
    /// Full credentials for authenticated calls.
    pub fn new(
        application_key: impl Into<String>,
        application_secret: impl Into<String>,
        consumer_key: impl Into<String>,
    ) -> Self {
        Self {
            application_key: application_key.into(),
            application_secret: application_secret.into(),
            consumer_key: Some(consumer_key.into()),
        }
    }

    /// Application-only credentials (no consumer key); suitable for public
    /// routes or for bootstrapping a credential request.
    pub fn application_only(
        application_key: impl Into<String>,
        application_secret: impl Into<String>,
    ) -> Self {
        Self {
            application_key: application_key.into(),
            application_secret: application_secret.into(),
            consumer_key: None,
        }
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("application_key", &self.application_key)
            .field("application_secret", &"<redacted>")
            .field(
                "consumer_key",
                &self.consumer_key.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// OAuth2 service-account credentials for the OVHcloud API.
///
/// Create a service account via `POST /me/api/oauth2/client` (see OVH docs:
/// "Managing OVHcloud service accounts via the API") to obtain a `client_id`
/// and `client_secret`. The client automatically fetches and refreshes Bearer
/// tokens using the OAuth2 client-credentials flow.
#[derive(Clone)]
pub struct OAuth2Credentials {
    /// OAuth2 client ID.
    pub client_id: String,
    /// OAuth2 client secret — never sent on the API data plane, only to the
    /// token endpoint.
    pub client_secret: String,
}

impl OAuth2Credentials {
    pub fn new(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
        }
    }
}

impl std::fmt::Debug for OAuth2Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuth2Credentials")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .finish()
    }
}

/// Response from the OVH OAuth2 token endpoint.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TokenResponse {
    pub access_token: String,
    #[allow(dead_code)]
    pub token_type: String,
    pub expires_in: u64,
    #[allow(dead_code)]
    pub scope: Option<String>,
}

/// A cached Bearer token together with its absolute expiry instant.
pub(crate) struct CachedToken {
    pub access_token: String,
    pub expires_at: std::time::Instant,
}

impl CachedToken {
    /// Build from a fresh [`TokenResponse`]. A 30-second safety margin avoids
    /// using a token that is about to expire mid-request.
    pub fn from_response(resp: &TokenResponse) -> Self {
        let duration = std::time::Duration::from_secs(resp.expires_in.saturating_sub(30));
        Self {
            access_token: resp.access_token.clone(),
            expires_at: std::time::Instant::now() + duration,
        }
    }

    pub fn is_valid(&self) -> bool {
        std::time::Instant::now() < self.expires_at
    }
}

/// Selects which authentication scheme the client uses.
#[derive(Clone, Debug)]
pub enum AuthMethod {
    /// Classic OVH signed-request auth (AK/AS/CK).
    Classic(Credentials),
    /// OAuth2 client-credentials flow (service accounts, IAM-compatible).
    OAuth2(OAuth2Credentials),
}

impl From<Credentials> for AuthMethod {
    fn from(creds: Credentials) -> Self {
        AuthMethod::Classic(creds)
    }
}

impl From<OAuth2Credentials> for AuthMethod {
    fn from(creds: OAuth2Credentials) -> Self {
        AuthMethod::OAuth2(creds)
    }
}

/// Compute the `X-Ovh-Signature` value for one request.
///
/// `url` must be the exact URL that will be sent on the wire (scheme, host,
/// path and query), and `body` the exact request body (empty string for
/// bodyless requests).
pub(crate) fn signature(
    application_secret: &str,
    consumer_key: &str,
    method: &str,
    url: &str,
    body: &str,
    timestamp: i64,
) -> String {
    let mut hasher = Sha1::new();
    hasher.update(application_secret.as_bytes());
    hasher.update(b"+");
    hasher.update(consumer_key.as_bytes());
    hasher.update(b"+");
    hasher.update(method.as_bytes());
    hasher.update(b"+");
    hasher.update(url.as_bytes());
    hasher.update(b"+");
    hasher.update(body.as_bytes());
    hasher.update(b"+");
    hasher.update(timestamp.to_string().as_bytes());
    format!("$1${}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Oracles below were produced independently with:
    //   printf '%s' '<secret>+<ck>+<method>+<url>+<body>+<ts>' | sha1sum

    #[test]
    fn signature_get_empty_body() {
        let sig = signature(
            "secret",
            "ck",
            "GET",
            "https://eu.api.ovh.com/1.0/allDom",
            "",
            1_700_000_000,
        );
        assert_eq!(sig, "$1$16ba58f5431d50160410dc18090bb424369248a0");
    }

    #[test]
    fn signature_post_with_body() {
        let sig = signature(
            "app_secret",
            "consumer",
            "POST",
            "https://eu.api.ovh.com/1.0/allDom/srv/serviceInfos",
            r#"{"renew":{"automatic":true}}"#,
            1_457_018_875,
        );
        assert_eq!(sig, "$1$5ff40d5fc6edb77d1e7a93fe7cb6680d8f251905");
    }

    #[test]
    fn signature_empty_consumer_key() {
        let sig = signature(
            "app_secret",
            "",
            "GET",
            "https://eu.api.ovh.com/1.0/allDom",
            "",
            1_457_018_875,
        );
        assert_eq!(sig, "$1$3900a7ed5b8e4aeb865c19411aa2b1d66c5994c7");
    }

    #[test]
    fn debug_redacts_secrets() {
        let creds = Credentials::new("app-key", "super-secret", "consumer-key");
        let rendered = format!("{creds:?}");
        assert!(rendered.contains("app-key"));
        assert!(!rendered.contains("super-secret"));
        assert!(!rendered.contains("consumer-key"));
    }

    #[test]
    fn oauth2_debug_redacts_client_secret() {
        let creds = OAuth2Credentials::new("my-client-id", "top-secret");
        let rendered = format!("{creds:?}");
        assert!(rendered.contains("my-client-id"));
        assert!(!rendered.contains("top-secret"));
    }

    #[test]
    fn cached_token_expiry() {
        let resp = TokenResponse {
            access_token: "abc".into(),
            token_type: "Bearer".into(),
            expires_in: 3600,
            scope: Some("all".into()),
        };
        let cached = CachedToken::from_response(&resp);
        assert!(cached.is_valid());
        assert_eq!(cached.access_token, "abc");
    }

    #[test]
    fn cached_token_zero_expiry_is_expired() {
        let resp = TokenResponse {
            access_token: "abc".into(),
            token_type: "Bearer".into(),
            expires_in: 0,
            scope: None,
        };
        let cached = CachedToken::from_response(&resp);
        assert!(!cached.is_valid());
    }

    #[test]
    fn auth_method_from_credentials() {
        let creds = Credentials::new("k", "s", "c");
        let method: AuthMethod = creds.into();
        assert!(matches!(method, AuthMethod::Classic(_)));
    }

    #[test]
    fn auth_method_from_oauth2() {
        let creds = OAuth2Credentials::new("id", "secret");
        let method: AuthMethod = creds.into();
        assert!(matches!(method, AuthMethod::OAuth2(_)));
    }
}
