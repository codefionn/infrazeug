//! KMS recipient stub (`INFRZEUG_KMS_SEED`) and config for future cloud backends.

use async_trait::async_trait;
use hkdf::Hkdf;
use infrazeug_secrets::kek::{challenge, open, seal};
use infrazeug_secrets::provider::{Provider, ProviderKind, RecipientEntry, WrapCtx};
use infrazeug_secrets::{Result, SecretsError};
use sha2::{Digest, Sha256};
use std::env;

const KEK_INFO: &[u8] = b"infrazeug-kms-hkdf-v1";

#[derive(Clone, Debug)]
pub struct KmsConfig {
    pub key_id: String,
}

/// Environment-backed KMS stub for tests (`INFRZEUG_KMS_SEED` hex or UTF-8).
pub struct EnvKmsProvider {
    config: KmsConfig,
    seed: Vec<u8>,
}

impl EnvKmsProvider {
    pub fn from_env(config: KmsConfig) -> Result<Self> {
        let raw = env::var("INFRZEUG_KMS_SEED")
            .map_err(|_| SecretsError::Provider("INFRZEUG_KMS_SEED not set".into()))?;
        let seed = hex::decode(raw.trim()).unwrap_or_else(|_| raw.into_bytes());
        if seed.is_empty() {
            return Err(SecretsError::Provider("empty KMS seed".into()));
        }
        Ok(Self { config, seed })
    }

    fn derive(&self, challenge: &[u8], data_key_id: &str) -> [u8; 32] {
        let mut ikm = self.seed.clone();
        ikm.extend_from_slice(self.config.key_id.as_bytes());
        ikm.extend_from_slice(challenge);
        let prk = Sha256::digest(&ikm);
        let hk = Hkdf::<Sha256>::new(Some(KEK_INFO), &prk);
        let mut okm = [0u8; 32];
        hk.expand(data_key_id.as_bytes(), &mut okm)
            .expect("hkdf expand");
        okm
    }
}

#[async_trait]
impl Provider for EnvKmsProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Kms
    }

    async fn wrap(&self, dek: &[u8; 32], ctx: &WrapCtx, label: &str) -> Result<RecipientEntry> {
        let ch = challenge(&ctx.data_key_id, &ctx.file_salt);
        let kek = self.derive(&ch, &ctx.data_key_id);
        let wrapped_key = seal(&kek, dek)?;
        Ok(RecipientEntry {
            kind: ProviderKind::Kms,
            label: label.to_string(),
            wrapped_key,
            params: serde_json::json!({
                "key_id": self.config.key_id,
                "backend": "env",
            }),
        })
    }

    async fn unwrap(&self, entry: &RecipientEntry, ctx: &WrapCtx) -> Result<Vec<u8>> {
        let ch = challenge(&ctx.data_key_id, &ctx.file_salt);
        let kek = self.derive(&ch, &ctx.data_key_id);
        open(&kek, &entry.wrapped_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_secrets::{create_envelope, generate_dek, unlock_envelope};

    #[tokio::test]
    async fn kms_env_roundtrip() {
        std::env::set_var("INFRZEUG_KMS_SEED", "test-seed-material");
        let provider = EnvKmsProvider::from_env(KmsConfig {
            key_id: "alias/prod".into(),
        })
        .unwrap();
        let dek = generate_dek();
        let envelope = create_envelope("prod", &dek, &provider, "aws")
            .await
            .unwrap();
        let entry = &envelope.recipients[0];
        let unlocked = unlock_envelope(&envelope, &provider, entry).await.unwrap();
        assert_eq!(&*unlocked, &dek);
    }
}
