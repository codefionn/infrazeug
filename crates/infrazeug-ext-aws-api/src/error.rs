//! Error type for the AWS API client.

/// Result alias for fallible AWS API operations.
pub type Result<T> = std::result::Result<T, AwsError>;

/// Anything that can go wrong while talking to AWS APIs.
#[derive(Debug, thiserror::Error)]
pub enum AwsError {
    #[error("aws http transport error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("aws api error {status}: {message}")]
    Api { status: u16, message: String },

    #[error("aws xml error: {0}")]
    Xml(String),

    #[error("aws json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("aws auth error: {0}")]
    Auth(String),
}
