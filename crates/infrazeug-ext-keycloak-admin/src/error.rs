//! Error type for the Keycloak Admin REST API client.

/// Result alias for fallible Keycloak admin operations.
pub type Result<T> = std::result::Result<T, KeycloakError>;

/// Anything that can go wrong while talking to the Keycloak Admin REST API.
#[derive(Debug, thiserror::Error)]
pub enum KeycloakError {
    /// An HTTP transport error (DNS, TLS, connection, timeout, …).
    #[error("keycloak http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// The API answered with a non-2xx status.
    #[error("keycloak api error {status}: {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Keycloak error discriminator, when present.
        error: Option<String>,
        /// Human-readable error description.
        message: String,
    },

    /// A response body or request body could not be (de)serialized.
    #[error("keycloak json error: {0}")]
    Json(#[from] serde_json::Error),

    /// The configured base URL is invalid.
    #[error("invalid keycloak url: {0}")]
    Url(String),

    /// Token acquisition or refresh failed.
    #[error("keycloak auth error: {0}")]
    Auth(String),
}
