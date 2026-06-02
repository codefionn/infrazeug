//! Error type for the Cloudflare API client.

/// Result alias for fallible Cloudflare API operations.
pub type Result<T> = std::result::Result<T, CloudflareError>;

/// Anything that can go wrong while talking to the Cloudflare API.
#[derive(Debug, thiserror::Error)]
pub enum CloudflareError {
    /// The HTTP request could not be sent or the response could not be read.
    #[error("cloudflare http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// The API answered with `success: false` or a non-2xx status.
    #[error("cloudflare api error {status}: {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Cloudflare error codes from the `errors` array, when present.
        codes: Vec<u64>,
        /// Human-readable error message(s).
        message: String,
    },

    /// A response body or request body could not be (de)serialized.
    #[error("cloudflare json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Auth configuration failed.
    #[error("cloudflare auth error: {0}")]
    Auth(String),
}
