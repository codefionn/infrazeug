//! Error type for the STACKIT IaaS API client.

/// Result alias for fallible STACKIT API operations.
pub type Result<T> = std::result::Result<T, StackitError>;

/// Anything that can go wrong while talking to the STACKIT IaaS API.
#[derive(Debug, thiserror::Error)]
pub enum StackitError {
    /// The HTTP request could not be sent or the response could not be read.
    #[error("stackit http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// The API answered with a non-2xx status.
    #[error("stackit api error {status}: {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Human-readable error message.
        message: String,
    },

    /// A response body or request body could not be (de)serialized.
    #[error("stackit json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Token acquisition or auth configuration failed.
    #[error("stackit auth error: {0}")]
    Auth(String),
}
