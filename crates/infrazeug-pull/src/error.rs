use thiserror::Error;

pub type Result<T> = std::result::Result<T, PullError>;

#[derive(Debug, Error)]
pub enum PullError {
    #[error("bootstrap: {0}")]
    Bootstrap(String),
    #[error("store: {0}")]
    Store(String),
    #[error("sealed plan: {0}")]
    Sealed(String),
    #[error("signature: {0}")]
    Signature(String),
    #[error("revoked")]
    Revoked,
    #[error("{0}")]
    Other(String),
}

impl From<infrazeug_core::CoreError> for PullError {
    fn from(e: infrazeug_core::CoreError) -> Self {
        PullError::Other(e.to_string())
    }
}

impl From<infrazeug_secrets::SecretsError> for PullError {
    fn from(e: infrazeug_secrets::SecretsError) -> Self {
        PullError::Sealed(e.to_string())
    }
}
