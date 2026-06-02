use thiserror::Error;

pub type Result<T> = std::result::Result<T, ShellError>;

#[derive(Debug, Error)]
pub enum ShellError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}
