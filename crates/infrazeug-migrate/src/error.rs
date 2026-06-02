use thiserror::Error;

#[derive(Debug, Error)]
pub enum MigrateError {
    #[error("ansible vault decrypt: {0}")]
    AnsibleDecrypt(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(String),
    #[error("secrets: {0}")]
    Secrets(#[from] infrazeug_secrets::SecretsError),
    #[error("{0}")]
    Other(String),
}
