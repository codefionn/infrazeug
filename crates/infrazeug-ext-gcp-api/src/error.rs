//! Error type for the GCP API client.

pub type Result<T> = std::result::Result<T, GcpError>;

#[derive(Debug, thiserror::Error)]
pub enum GcpError {
    #[error("gcp http transport error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("gcp api error {status}: {message}")]
    Api { status: u16, message: String },

    #[error("gcp json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("gcp auth error: {0}")]
    Auth(String),
}
