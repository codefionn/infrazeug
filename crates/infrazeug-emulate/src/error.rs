use thiserror::Error;

pub type Result<T> = std::result::Result<T, EmulateError>;

#[derive(Debug, Error)]
pub enum EmulateError {
    #[error("build graph cycle: {0}")]
    Cycle(String),

    #[error("unknown container spec id: {0}")]
    UnknownSpec(String),

    #[error("lock drift: {0}")]
    LockDrift(String),

    #[error("builder OnMachine is not implemented in M3")]
    OnMachineBuilder,

    #[error("mount secret vault requires M4 secrets")]
    VaultSecretMount,

    #[error("like target must be an emulated kind (Local, Container, MicroVm)")]
    LikeNotEmulated,

    #[error("{0}")]
    Other(String),
}

impl EmulateError {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

impl From<std::io::Error> for EmulateError {
    fn from(e: std::io::Error) -> Self {
        Self::Other(e.to_string())
    }
}

impl From<serde_json::Error> for EmulateError {
    fn from(e: serde_json::Error) -> Self {
        Self::Other(e.to_string())
    }
}

impl From<toml::de::Error> for EmulateError {
    fn from(e: toml::de::Error) -> Self {
        Self::Other(e.to_string())
    }
}

impl From<toml::ser::Error> for EmulateError {
    fn from(e: toml::ser::Error) -> Self {
        Self::Other(e.to_string())
    }
}
