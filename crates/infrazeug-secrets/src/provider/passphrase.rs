use super::{Provider, ProviderKind, RecipientEntry, WrapCtx};
use crate::error::{Result, SecretsError};
use argon2::{Algorithm, Argon2, Params, Version};
use async_trait::async_trait;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct PassphraseProvider {
    passphrase: Zeroizing<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PassphraseParams {
    salt: [u8; 16],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
}

impl PassphraseProvider {
    pub fn new(passphrase: impl Into<String>) -> Self {
        Self {
            passphrase: Zeroizing::new(passphrase.into()),
        }
    }

    fn derive_kek(&self, params: &PassphraseParams) -> Result<Zeroizing<[u8; 32]>> {
        let argon = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(params.m_cost, params.t_cost, params.p_cost, Some(32))
                .map_err(|e| SecretsError::Provider(e.to_string()))?,
        );
        let mut out = Zeroizing::new([0u8; 32]);
        argon
            .hash_password_into(self.passphrase.as_bytes(), &params.salt, &mut out[..])
            .map_err(|e| SecretsError::Provider(e.to_string()))?;
        Ok(out)
    }

    /// AEAD associated data binding the wrapped DEK to its envelope context
    /// (data key id + file salt), so a recipient blob cannot be lifted into a
    /// different envelope without detection.
    fn aad(ctx: &WrapCtx) -> Vec<u8> {
        crate::kek::challenge(&ctx.data_key_id, &ctx.file_salt)
    }

    fn seal(&self, kek: &[u8; 32], dek: &[u8; 32], aad: &[u8]) -> Result<Vec<u8>> {
        let mut nonce = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce);
        let key = Key::from_slice(kek);
        let cipher = XChaCha20Poly1305::new(key);
        let ct = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: dek.as_ref(),
                    aad,
                },
            )
            .map_err(|_| SecretsError::Decrypt)?;
        let mut out = nonce.to_vec();
        out.extend(ct);
        Ok(out)
    }

    fn open(&self, kek: &[u8; 32], blob: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        if blob.len() < 24 {
            return Err(SecretsError::Provider("wrapped dek short".into()));
        }
        let (nonce, ct) = blob.split_at(24);
        let key = Key::from_slice(kek);
        let cipher = XChaCha20Poly1305::new(key);
        cipher
            .decrypt(
                XNonce::from_slice(nonce),
                chacha20poly1305::aead::Payload { msg: ct, aad },
            )
            .map_err(|_| SecretsError::Decrypt)
    }
}

#[async_trait]
impl Provider for PassphraseProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Passphrase
    }

    async fn wrap(&self, dek: &[u8; 32], ctx: &WrapCtx, label: &str) -> Result<RecipientEntry> {
        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        let params = PassphraseParams {
            salt,
            m_cost: 256 * 1024,
            t_cost: 3,
            p_cost: 4,
        };
        let kek = self.derive_kek(&params)?;
        let wrapped_key = self.seal(&kek, dek, &Self::aad(ctx))?;
        Ok(RecipientEntry {
            kind: ProviderKind::Passphrase,
            label: label.to_string(),
            wrapped_key,
            params: serde_json::to_value(&params).unwrap_or(serde_json::Value::Null),
        })
    }

    async fn unwrap(&self, entry: &RecipientEntry, ctx: &WrapCtx) -> Result<Vec<u8>> {
        let params: PassphraseParams = serde_json::from_value(entry.params.clone())
            .map_err(|e| SecretsError::Provider(e.to_string()))?;
        let kek = self.derive_kek(&params)?;
        self.open(&kek, &entry.wrapped_key, &Self::aad(ctx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(id: &str, salt_byte: u8) -> WrapCtx {
        WrapCtx {
            data_key_id: id.to_string(),
            file_salt: [salt_byte; 32],
        }
    }

    #[tokio::test]
    async fn wrap_bound_to_envelope_context() {
        let dek = [9u8; 32];
        let p = PassphraseProvider::new("pw");
        let a = ctx("prod", 1);
        let entry = p.wrap(&dek, &a, "recovery").await.unwrap();

        // Same context unwraps.
        assert_eq!(p.unwrap(&entry, &a).await.unwrap(), dek.to_vec());

        // Lifting the recipient into a different envelope (id or file_salt)
        // fails: the AAD no longer matches.
        assert!(p.unwrap(&entry, &ctx("ops", 1)).await.is_err());
        assert!(p.unwrap(&entry, &ctx("prod", 2)).await.is_err());
    }
}
