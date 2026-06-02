//! PKCS#11 / PIV recipient (fixture PIN + slot for CI).

use async_trait::async_trait;
use hkdf::Hkdf;
use infrazeug_secrets::kek::{challenge, open, seal};
use infrazeug_secrets::provider::{Provider, ProviderKind, RecipientEntry, WrapCtx};
use infrazeug_secrets::Result;
use sha2::{Digest, Sha256};

const KEK_INFO: &[u8] = b"infrazeug-pkcs11-hkdf-v1";

pub struct Pkcs11Provider {
    pub slot: String,
    pub pin: String,
}

impl Pkcs11Provider {
    pub fn new(slot: impl Into<String>, pin: impl Into<String>) -> Self {
        Self {
            slot: slot.into(),
            pin: pin.into(),
        }
    }

    fn token_mac(&self, challenge: &[u8]) -> [u8; 32] {
        let mut material = Vec::new();
        material.extend_from_slice(self.slot.as_bytes());
        material.push(0);
        material.extend_from_slice(self.pin.as_bytes());
        material.extend_from_slice(challenge);
        let digest = Sha256::digest(&material);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest[..32]);
        out
    }

    fn derive_kek(&self, mac: &[u8; 32], data_key_id: &str) -> [u8; 32] {
        let hk = Hkdf::<Sha256>::new(Some(KEK_INFO), mac);
        let mut okm = [0u8; 32];
        hk.expand(data_key_id.as_bytes(), &mut okm)
            .expect("hkdf expand");
        okm
    }
}

#[async_trait]
impl Provider for Pkcs11Provider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Pkcs11
    }

    async fn wrap(&self, dek: &[u8; 32], ctx: &WrapCtx, label: &str) -> Result<RecipientEntry> {
        let ch = challenge(&ctx.data_key_id, &ctx.file_salt);
        let mac = self.token_mac(&ch);
        let kek = self.derive_kek(&mac, &ctx.data_key_id);
        let wrapped_key = seal(&kek, dek)?;
        Ok(RecipientEntry {
            kind: ProviderKind::Pkcs11,
            label: label.to_string(),
            wrapped_key,
            params: serde_json::json!({ "slot": self.slot }),
        })
    }

    async fn unwrap(&self, entry: &RecipientEntry, ctx: &WrapCtx) -> Result<Vec<u8>> {
        let ch = challenge(&ctx.data_key_id, &ctx.file_salt);
        let mac = self.token_mac(&ch);
        let kek = self.derive_kek(&mac, &ctx.data_key_id);
        open(&kek, &entry.wrapped_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_secrets::{create_envelope, generate_dek, unlock_envelope};

    #[tokio::test]
    async fn pkcs11_roundtrip() {
        let dek = generate_dek();
        let provider = Pkcs11Provider::new("9a", "9999");
        let envelope = create_envelope("ops", &dek, &provider, "piv-a")
            .await
            .unwrap();
        let entry = &envelope.recipients[0];
        let unlocked = unlock_envelope(&envelope, &provider, entry).await.unwrap();
        assert_eq!(unlocked.as_ref(), &dek);
    }
}
