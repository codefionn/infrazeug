//! Error type for the Backblaze B2 Native API client.

/// Result alias for fallible B2 API operations.
pub type Result<T> = std::result::Result<T, BackblazeError>;

/// Anything that can go wrong while talking to the B2 Native API.
#[derive(Debug, thiserror::Error)]
pub enum BackblazeError {
    /// The HTTP request could not be sent or the response could not be read.
    #[error("backblaze b2 http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// The API answered with a non-2xx status or a B2 error payload.
    #[error("backblaze b2 api error {status} ({code}): {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// B2 error code (e.g. `expired_auth_token`, `duplicate_bucket_name`).
        code: String,
        /// Human-readable error message.
        message: String,
    },

    /// A response body or request body could not be (de)serialized.
    #[error("backblaze b2 json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Auth configuration failed.
    #[error("backblaze b2 auth error: {0}")]
    Auth(String),
}

impl BackblazeError {
    pub(crate) fn is_auth_token_error(&self) -> bool {
        matches!(
            self,
            Self::Api {
                code,
                status: 401,
                ..
            } if code == "bad_auth_token" || code == "expired_auth_token"
        )
    }
}
