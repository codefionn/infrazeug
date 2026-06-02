use thiserror::Error;

pub type Result<T> = std::result::Result<T, NativeError>;

#[derive(Debug, Error)]
pub enum NativeError {
    #[error("native method `{method}` not found")]
    NotFound { method: String },
    #[error("invalid input for method `{method}`: {detail}")]
    InvalidInput { method: String, detail: String },
    #[error("{0}")]
    Other(String),
}

impl NativeError {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
