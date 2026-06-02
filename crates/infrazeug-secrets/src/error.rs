use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("invalid vault envelope: {0}")]
    Format(String),
    #[error("decryption failed")]
    Decrypt,
    #[error("encryption failed")]
    Encrypt,
    #[error("io: {0}")]
    Io(String),
    #[error("data key {0} not unlocked")]
    Locked(String),
    #[error("field {field} missing in vault files: {files:?}")]
    MissingField { field: String, files: Vec<String> },
    #[error("backend conflict on {key}")]
    Conflict { key: String },
    #[error("backend: {0}")]
    Backend(String),
    #[error("provider: {0}")]
    Provider(String),
    #[error("signature verification failed")]
    BadSignature,
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SecretsError>;

impl From<serde_cbor::Error> for SecretsError {
    fn from(e: serde_cbor::Error) -> Self {
        SecretsError::Format(e.to_string())
    }
}
