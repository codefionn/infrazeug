pub type Result<T> = std::result::Result<T, AzureError>;

#[derive(Debug, thiserror::Error)]
pub enum AzureError {
    #[error("azure http transport error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("azure api error {status}: {message}")]
    Api { status: u16, message: String },

    #[error("azure json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("azure auth error: {0}")]
    Auth(String),
}
