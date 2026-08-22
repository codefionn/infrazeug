//! Errors returned by the NetBird client.

/// Result alias for NetBird API operations.
pub type Result<T> = std::result::Result<T, NetBirdError>;

/// Anything that can go wrong while talking to NetBird.
#[derive(Debug, thiserror::Error)]
pub enum NetBirdError {
    /// The request could not be sent or the response could not be read.
    #[error("netbird http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// NetBird returned a non-success HTTP response.
    #[error("netbird api error {status}: {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Message extracted from NetBird's JSON error body, or the raw body.
        message: String,
        /// The unmodified response body, when it was non-empty.
        body: Option<String>,
    },

    /// A response body or request body could not be serialized or decoded.
    #[error("netbird json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Authentication configuration could not be represented as an HTTP header.
    #[error("netbird auth error: {0}")]
    Auth(String),
}
