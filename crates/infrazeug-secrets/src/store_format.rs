//! Vault store layout and on-disk version constants (SOUL §6.2).

use crate::error::{Result, SecretsError};
use serde::{Deserialize, Serialize};

/// Store-level format written to `meta/store.cbor`.
///
/// v3 hardens backend key validation, private local file creation, and migrates
/// existing vault-file wrappers to [`VAULT_FILE_VERSION`] without decrypting.
pub const STORE_FORMAT_VERSION: u32 = 3;

pub const META_KEY: &str = "meta/store.cbor";

/// Magic + version prefix for `keys/<id>.dkey` wire encoding (v1).
pub const DKEY_MAGIC: &[u8; 8] = b"INFRZDK1";
pub const DKEY_WIRE_VERSION: u8 = 0x01;

/// Vault file blob version byte written after `INFRZVLT`. See [`crate::format`].
pub const VAULT_FILE_VERSION: u8 = 0x02;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreMeta {
    pub format_version: u32,
}

impl StoreMeta {
    pub fn current() -> Self {
        Self {
            format_version: STORE_FORMAT_VERSION,
        }
    }
}

pub fn encode_store_meta(meta: &StoreMeta) -> Result<Vec<u8>> {
    Ok(serde_cbor::to_vec(meta)?)
}

pub fn decode_store_meta(bytes: &[u8]) -> Result<StoreMeta> {
    serde_cbor::from_slice(bytes).map_err(|e| SecretsError::Format(e.to_string()))
}

pub fn is_wrapped_dkey_blob(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[0..8] == DKEY_MAGIC
}
