use thiserror::Error;

pub type Result<T> = std::result::Result<T, TransportError>;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("{0}")]
    Other(String),
}
