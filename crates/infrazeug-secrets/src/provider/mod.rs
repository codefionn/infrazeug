mod passphrase;

pub use passphrase::PassphraseProvider;

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    Passphrase,
    SshAgent,
    Fido2,
    Pkcs11,
    Age,
    Kms,
}

#[derive(Clone, Debug)]
pub struct WrapCtx {
    pub data_key_id: String,
    pub file_salt: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipientEntry {
    pub kind: ProviderKind,
    pub label: String,
    pub wrapped_key: Vec<u8>,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn kind(&self) -> ProviderKind;
    async fn wrap(&self, dek: &[u8; 32], ctx: &WrapCtx, label: &str) -> Result<RecipientEntry>;
    async fn unwrap(&self, entry: &RecipientEntry, ctx: &WrapCtx) -> Result<Vec<u8>>;
}
