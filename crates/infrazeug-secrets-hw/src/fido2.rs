//! FIDO2 `hmac-secret` recipient (fixture for CI; real tokens via same challenge path).

use async_trait::async_trait;
use hkdf::Hkdf;
use infrazeug_secrets::kek::{challenge, open, seal};
use infrazeug_secrets::provider::{Provider, ProviderKind, RecipientEntry, WrapCtx};
use infrazeug_secrets::{Result, SecretsError};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const KEK_INFO: &[u8] = b"infrazeug-fido2-hkdf-v1";

/// Deterministic FIDO2-style provider using credential id + PIN (tests and dev).
pub struct Fido2Provider {
    pub credential_id: String,
    pub pin: Zeroizing<String>,
}

/// Argon2id cost parameters matching `PassphraseProvider` (SOUL §6: PINs and
/// passphrases are low-entropy and must be stretched before key derivation).
const KDF_M_COST: u32 = 256 * 1024;
const KDF_T_COST: u32 = 3;
const KDF_P_COST: u32 = 4;

impl Fido2Provider {
    pub fn new(credential_id: impl Into<String>, pin: impl Into<String>) -> Self {
        Self {
            credential_id: credential_id.into(),
            pin: Zeroizing::new(pin.into()),
        }
    }

    /// Stretch the PIN with Argon2id over a per-recipient random salt.
    ///
    /// The fixture provider stands in for a hardware `hmac-secret` operation;
    /// without stretching, a captured `keys/<id>.dkey` could be brute-forced
    /// offline against the (typically 4-8 digit) PIN with one hash per guess.
    fn stretch_pin(&self, salt: &[u8; 16], m_cost: u32, t_cost: u32, p_cost: u32) -> Result<[u8; 32]> {
        use argon2::{Algorithm, Argon2, Params, Version};
        let argon = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(m_cost, t_cost, p_cost, Some(32))
                .map_err(|e| SecretsError::Provider(e.to_string()))?,
        );
        let mut out = [0u8; 32];
        argon
            .hash_password_into(self.pin.as_bytes(), salt, &mut out)
            .map_err(|e| SecretsError::Provider(e.to_string()))?;
        Ok(out)
    }

    fn hmac_secret(&self, pin_key: &[u8], challenge: &[u8]) -> Result<[u8; 32]> {
        let mut material = Vec::new();
        material.extend_from_slice(self.credential_id.as_bytes());
        material.push(0);
        material.extend_from_slice(pin_key);
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

    /// Pin-derived secret input for a recipient entry. Current entries carry
    /// `kdf: "argon2id-v1"` and a salt; pre-hardening entries (no `kdf` param)
    /// fall back to the legacy unstretched PIN so existing fixture vaults still
    /// open — those should be re-created (`vault recipients-add`) to gain the
    /// stretching.
    fn entry_pin_key(&self, entry: &Option<serde_json::Value>) -> Result<Vec<u8>> {
        let params = entry.clone().unwrap_or(serde_json::Value::Null);
        if params.get("kdf").and_then(|v| v.as_str()) == Some("argon2id-v1") {
            let salt_hex = params
                .get("kdf_salt")
                .and_then(|v| v.as_str())
                .ok_or_else(|| SecretsError::Provider("fido2 recipient missing kdf_salt".into()))?;
            let salt = hex16(salt_hex)?;
            let m = kdf_cost(&params, "kdf_m", KDF_M_COST)?;
            let t = kdf_cost(&params, "kdf_t", KDF_T_COST)?;
            let p = kdf_cost(&params, "kdf_p", KDF_P_COST)?;
            Ok(self.stretch_pin(&salt, m, t, p)?.to_vec())
        } else {
            // Legacy (unstretched) derivation — kept only for unlock compat.
            Ok(self.pin.as_bytes().to_vec())
        }
    }
}

fn hex16(s: &str) -> Result<[u8; 16]> {
    let bytes =
        hex::decode(s).map_err(|e| SecretsError::Provider(format!("bad kdf_salt: {e}")))?;
    let arr: [u8; 16] = bytes
        .try_into()
        .map_err(|_| SecretsError::Provider("kdf_salt must be 16 bytes".into()))?;
    Ok(arr)
}

fn kdf_cost(params: &serde_json::Value, key: &str, default: u32) -> Result<u32> {
    match params.get(key) {
        None => Ok(default),
        Some(v) => v
            .as_u64()
            .map(|n| n as u32)
            .ok_or_else(|| SecretsError::Provider(format!("bad {key}"))),
    }
}

#[async_trait]
impl Provider for Fido2Provider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Fido2
    }

    async fn wrap(&self, dek: &[u8; 32], ctx: &WrapCtx, label: &str) -> Result<RecipientEntry> {
        let mut salt = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut salt);
        let pin_key = self.stretch_pin(&salt, KDF_M_COST, KDF_T_COST, KDF_P_COST)?;
        let ch = challenge(&ctx.data_key_id, &ctx.file_salt);
        let secret = self.hmac_secret(&pin_key, &ch)?;
        let kek = self.derive_kek(&secret, &ctx.data_key_id);
        let wrapped_key = seal(&kek, dek, &ch)?;
        Ok(RecipientEntry {
            kind: ProviderKind::Fido2,
            label: label.to_string(),
            wrapped_key,
            params: serde_json::json!({
                "credential_id": self.credential_id,
                "resident": false,
                "kdf": "argon2id-v1",
                "kdf_salt": hex::encode(salt),
                "kdf_m": KDF_M_COST,
                "kdf_t": KDF_T_COST,
                "kdf_p": KDF_P_COST,
            }),
        })
    }

    async fn unwrap(&self, entry: &RecipientEntry, ctx: &WrapCtx) -> Result<Vec<u8>> {
        let pin_key = self.entry_pin_key(&Some(entry.params.clone()))?;
        let ch = challenge(&ctx.data_key_id, &ctx.file_salt);
        let secret = self.hmac_secret(&pin_key, &ch)?;
        let kek = self.derive_kek(&secret, &ctx.data_key_id);
        open(&kek, &entry.wrapped_key, &ch)
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
        assert_eq!(
            entry.params.get("kdf").and_then(|v| v.as_str()),
            Some("argon2id-v1"),
            "new recipients must use stretched PIN derivation"
        );
        let unlocked = unlock_envelope(&envelope, &provider, entry).await.unwrap();
        assert_eq!(unlocked.as_ref(), &dek);
    }

    #[tokio::test]
    async fn fido2_wrong_pin_fails() {
        let dek = generate_dek();
        let provider = Fido2Provider::new("cred-test", "1234");
        let envelope = create_envelope("prod", &dek, &provider, "yubikey-a")
            .await
            .unwrap();
        let wrong = Fido2Provider::new("cred-test", "9999");
        assert!(unlock_envelope(&envelope, &wrong, &envelope.recipients[0])
            .await
            .is_err());
    }

    #[tokio::test]
    async fn fido2_legacy_recipient_still_unwraps() {
        // Pre-hardening recipients (no kdf params) used the raw PIN; they must
        // remain unlockable so existing fixture vaults are not bricked.
        let dek = generate_dek();
        let provider = Fido2Provider::new("cred-test", "1234");
        let ch = challenge("prod", &[1u8; 32]);
        let secret = provider
            .hmac_secret(provider.pin.as_bytes(), &ch)
            .unwrap();
        let kek = provider.derive_kek(&secret, "prod");
        let entry = RecipientEntry {
            kind: ProviderKind::Fido2,
            label: "legacy".into(),
            // Legacy blobs were sealed without AAD binding.
            wrapped_key: infrazeug_secrets::kek::seal(&kek, &dek, &[]).unwrap(),
            params: serde_json::json!({ "credential_id": "cred-test", "resident": false }),
        };
        let ctx = WrapCtx {
            data_key_id: "prod".into(),
            file_salt: [1u8; 32],
        };
        let opened = provider.unwrap(&entry, &ctx).await.unwrap();
        assert_eq!(opened, dek.to_vec());
    }
}
