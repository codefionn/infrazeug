//! Real FIDO2 `hmac-secret` recipient (SOUL §6.3), gated behind `fido2-device`.
//!
//! Unlike [`crate::Fido2Provider`] (a deterministic fixture for CI), this talks
//! CTAP2 to a physical authenticator and uses the `hmac-secret` extension as the
//! key-encryption-key source. Both `wrap` and `unwrap` derive the same secret
//! because it is computed by the token from its per-credential `CredRandom` and a
//! fixed salt we store alongside the credential id:
//!
//! - **wrap**: enroll a non-resident credential with `hmac-secret`, generate a
//!   random 32-byte salt, ask the token for `HMAC(CredRandom, salt)` (one touch),
//!   derive `kek = HKDF(secret)`, and seal the DEK. The credential id and salt are
//!   stored in the recipient entry; the secret never leaves the token.
//! - **unwrap**: replay the assertion for the stored credential id and salt (one
//!   touch + PIN), recover the same secret, re-derive `kek`, and open the DEK.
//!
//! Enrollment and unlock both require user presence (touch) and, if the token
//! enforces user verification, the PIN.

use async_trait::async_trait;
use base64::Engine;
use ctap_hid_fido2::fidokey::{
    AssertionExtension as Gext, CredentialExtension as Mext, GetAssertionArgsBuilder,
    MakeCredentialArgsBuilder,
};
use ctap_hid_fido2::{verifier, Cfg, FidoKeyHid, FidoKeyHidFactory};
use hkdf::Hkdf;
use infrazeug_secrets::kek::{open, seal};
use infrazeug_secrets::provider::{Provider, ProviderKind, RecipientEntry, WrapCtx};
use infrazeug_secrets::{Result, SecretsError};
use rand::RngCore;
use sha2::Sha256;

const KEK_INFO: &[u8] = b"infrazeug-fido2-device-hkdf-v1";

/// Connection parameters for a FIDO2 authenticator.
#[derive(Clone)]
pub struct Fido2DeviceConfig {
    /// Relying-party id the credential is scoped to (e.g. `infrazeug.local`).
    /// Must match between enrollment and unlock.
    pub rp_id: String,
    /// Authenticator PIN, required when the token enforces user verification.
    pub pin: Option<String>,
}

impl Fido2DeviceConfig {
    pub fn new(rp_id: impl Into<String>) -> Self {
        Self {
            rp_id: rp_id.into(),
            pin: None,
        }
    }

    pub fn with_pin(mut self, pin: impl Into<String>) -> Self {
        self.pin = Some(pin.into());
        self
    }
}

/// Recipient backed by a FIDO2 authenticator's `hmac-secret` extension.
pub struct Fido2Device {
    cfg: Fido2DeviceConfig,
}

impl Fido2Device {
    pub fn new(cfg: Fido2DeviceConfig) -> Self {
        Self { cfg }
    }

    fn err(e: impl std::fmt::Display) -> SecretsError {
        SecretsError::Provider(format!("fido2: {e}"))
    }

    fn device() -> Result<FidoKeyHid> {
        FidoKeyHidFactory::create(&Cfg::init()).map_err(Self::err)
    }

    fn derive_kek(secret: &[u8; 32], data_key_id: &str) -> [u8; 32] {
        let hk = Hkdf::<Sha256>::new(Some(KEK_INFO), secret);
        let mut okm = [0u8; 32];
        hk.expand(data_key_id.as_bytes(), &mut okm)
            .expect("hkdf expand");
        okm
    }

    /// Run a `get_assertion` with `hmac-secret(salt)` and return the 32-byte secret.
    fn hmac_secret(
        &self,
        device: &FidoKeyHid,
        credential_id: &[u8],
        salt: &[u8; 32],
    ) -> Result<[u8; 32]> {
        let challenge = verifier::create_challenge();
        let ext = Gext::HmacSecret(Some(*salt));
        let mut builder = GetAssertionArgsBuilder::new(&self.cfg.rp_id, &challenge)
            .credential_id(credential_id)
            .extensions(&[ext]);
        if let Some(pin) = &self.cfg.pin {
            builder = builder.pin(pin);
        }
        let args = builder.build();
        let assertions = device.get_assertion_with_args(&args).map_err(Self::err)?;
        let assertion = assertions
            .into_iter()
            .next()
            .ok_or_else(|| Self::err("authenticator returned no assertion"))?;
        assertion
            .extensions
            .iter()
            .find_map(|e| match e {
                Gext::HmacSecret(Some(secret)) => Some(*secret),
                _ => None,
            })
            .ok_or_else(|| Self::err("authenticator returned no hmac-secret output"))
    }
}

#[async_trait]
impl Provider for Fido2Device {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Fido2
    }

    async fn wrap(&self, dek: &[u8; 32], ctx: &WrapCtx, label: &str) -> Result<RecipientEntry> {
        let device = Self::device()?;

        // 1. Enroll a (non-resident) credential with the hmac-secret extension.
        let challenge = verifier::create_challenge();
        let mut builder = MakeCredentialArgsBuilder::new(&self.cfg.rp_id, &challenge)
            .extensions(&[Mext::HmacSecret(Some(true))]);
        if let Some(pin) = &self.cfg.pin {
            builder = builder.pin(pin);
        }
        let attestation = device
            .make_credential_with_args(&builder.build())
            .map_err(Self::err)?;
        let credential_id = attestation.credential_descriptor.id.clone();

        // 2. Derive the hmac secret for a fresh salt, then wrap the DEK with it.
        let mut salt = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut salt);
        let secret = self.hmac_secret(&device, &credential_id, &salt)?;
        let kek = Self::derive_kek(&secret, &ctx.data_key_id);
        let wrapped_key = seal(&kek, dek)?;

        let b64 = base64::engine::general_purpose::STANDARD;
        Ok(RecipientEntry {
            kind: ProviderKind::Fido2,
            label: label.to_string(),
            wrapped_key,
            params: serde_json::json!({
                "device": "fido2",
                "rp_id": self.cfg.rp_id,
                "credential_id": b64.encode(&credential_id),
                "salt": b64.encode(salt),
                "resident": false,
            }),
        })
    }

    async fn unwrap(&self, entry: &RecipientEntry, ctx: &WrapCtx) -> Result<Vec<u8>> {
        let b64 = base64::engine::general_purpose::STANDARD;
        let credential_id = entry
            .params
            .get("credential_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Self::err("recipient entry missing credential_id"))?;
        let credential_id = b64.decode(credential_id).map_err(Self::err)?;
        let salt = entry
            .params
            .get("salt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Self::err("recipient entry missing salt"))?;
        let salt: [u8; 32] = b64
            .decode(salt)
            .map_err(Self::err)?
            .as_slice()
            .try_into()
            .map_err(|_| Self::err("stored salt is not 32 bytes"))?;

        let device = Self::device()?;
        let secret = self.hmac_secret(&device, &credential_id, &salt)?;
        let kek = Self::derive_kek(&secret, &ctx.data_key_id);
        open(&kek, &entry.wrapped_key)
    }
}
