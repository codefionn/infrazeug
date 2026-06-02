use serde::{Deserialize, Serialize};

/// Reference to a vault file and optional JSON field path (SOUL §3.9).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VaultRef {
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl VaultRef {
    pub fn file(path: impl Into<String>) -> Self {
        Self {
            file: path.into(),
            field: None,
        }
    }

    pub fn field(path: impl Into<String>, field: impl Into<String>) -> Self {
        Self {
            file: path.into(),
            field: Some(field.into()),
        }
    }

    /// Reference a generated, run-mutable vault file under `files/mutable/`.
    pub fn mutable_file(path: impl Into<String>) -> Self {
        Self::file(mutable_vault_path(path))
    }

    /// Reference a field in a generated, run-mutable vault file under `files/mutable/`.
    pub fn mutable_field(path: impl Into<String>, field: impl Into<String>) -> Self {
        Self::field(mutable_vault_path(path), field)
    }
}

pub const MUTABLE_VAULT_PREFIX: &str = "mutable/";

pub fn mutable_vault_path(path: impl Into<String>) -> String {
    let path = path.into();
    if path.starts_with(MUTABLE_VAULT_PREFIX) {
        path
    } else {
        format!("{MUTABLE_VAULT_PREFIX}{path}")
    }
}
