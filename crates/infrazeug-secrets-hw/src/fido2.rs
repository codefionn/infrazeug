//! FIDO2 `hmac-secret` recipient (fixture for CI; real tokens via same challenge path).

use async_trait::async_trait;
use hkdf::Hkdf;
use infrazeug_secrets::kek::{challenge, open, seal};
use infrazeug_secrets::provider::{Provider, ProviderKind, RecipientEntry, WrapCtx};
use infrazeug_secrets::Result;
use sha2::{Digest, Sha256};

const KEK_INFO: &[u8] = b"infrazeug-fido2-hkdf-v1";

/// Deterministic FIDO2-style provider using credential id + PIN (tests and dev).
pub struct Fido2Provider {
    pub credential_id: String,
    pub pin: String,
}

impl Fido2Provider {
    pub fn new(credential_id: impl Into<String>, pin: impl Into<String>) -> Self {
        Self {
            credential_id: credential_id.into(),
            pin: pin.into(),
        }
    }

    fn hmac_secret(&self, challenge: &[u8]) -> Result<[u8; 32]> {
        let mut material = Vec::new();
        material.extend_from_slice(self.credential_id.as_bytes());
        material.push(0);
        material.extend_from_slice(self.pin.as_bytes());
        material.extend_from_slice(challenge);
        let digest = Sha256::digest(&material);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest[..32]);
        Ok(out)
    }

    fn derive_kek(&self, secret: &[u8; 32], data_key_id: &str) -> [u8; 32] {
        let hk = Hkdf::<Sha256>::new(Some(KEK_INFO), secret);
        let mut okm = [0u8; 32];
        hk.expand(data_key_id.as_bytes(), &mut okm)
            .expect("hkdf expand");
        okm
    }
}

#[async_trait]
impl Provider for Fido2Provider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Fido2
    }

    async fn wrap(&self, dek: &[u8; 32], ctx: &WrapCtx, label: &str) -> Result<RecipientEntry> {
        let ch = challenge(&ctx.data_key_id, &ctx.file_salt);
        let secret = self.hmac_secret(&ch)?;
        let kek = self.derive_kek(&secret, &ctx.data_key_id);
        let wrapped_key = seal(&kek, dek)?;
        Ok(RecipientEntry {
            kind: ProviderKind::Fido2,
            label: label.to_string(),
            wrapped_key,
            params: serde_json::json!({
                "credential_id": self.credential_id,
                "resident": false,
            }),
        })
    }

    async fn unwrap(&self, entry: &RecipientEntry, ctx: &WrapCtx) -> Result<Vec<u8>> {
        let ch = challenge(&ctx.data_key_id, &ctx.file_salt);
        let secret = self.hmac_secret(&ch)?;
        let kek = self.derive_kek(&secret, &ctx.data_key_id);
        open(&kek, &entry.wrapped_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_secrets::{create_envelope, generate_dek, unlock_envelope};

    #[tokio::test]
    async fn fido2_roundtrip() {
        let dek = generate_dek();
        let provider = Fido2Provider::new("cred-test", "1234");
        let envelope = create_envelope("prod", &dek, &provider, "yubikey-a")
            .await
            .unwrap();
        let entry = &envelope.recipients[0];
        let unlocked = unlock_envelope(&envelope, &provider, entry).await.unwrap();
        assert_eq!(unlocked.as_ref(), &dek);
    }
}
