//! Error type for the IONOS Cloud API client.

/// Result alias for fallible IONOS Cloud API operations.
pub type Result<T> = std::result::Result<T, IonosError>;

/// Anything that can go wrong while talking to the IONOS Cloud API.
#[derive(Debug, thiserror::Error)]
pub enum IonosError {
    /// The HTTP request could not be sent or the response could not be read
    /// (DNS, TLS, connection reset, timeout, …).
    #[error("ionos http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// The API answered with a non-2xx status.
    #[error("ionos api error {status}: {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// IONOS `errorCode` values from the `messages` array, when present.
        codes: Vec<String>,
        /// Human-readable error message(s).
        message: String,
    },

    /// A response body or request body could not be (de)serialized.
    #[error("ionos json error: {0}")]
    Json(#[from] serde_json::Error),

    /// The configured endpoint or a built request URL was invalid.
    #[error("invalid ionos url: {0}")]
    Url(String),

    /// Token acquisition or auth configuration failed.
    #[error("ionos auth error: {0}")]
    Auth(String),
}
