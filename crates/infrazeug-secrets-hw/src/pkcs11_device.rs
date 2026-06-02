//! Real PKCS#11 token recipient (SOUL §6.3), gated behind `pkcs11-device`.
//!
//! Unlike [`crate::Pkcs11Provider`] (a deterministic fixture for CI), this talks
//! to an actual token via a PKCS#11 module. It uses an RSA hybrid so wrap and
//! unwrap are self-consistent:
//!
//! - **wrap**: generate a random 32-byte seed, RSA-encrypt it to the token's
//!   public key, derive `kek = HKDF(seed)`, and seal the DEK. The ciphertext is
//!   stored in the recipient entry.
//! - **unwrap**: RSA-decrypt the stored ciphertext on the token (requires the
//!   PIN), recover the seed, re-derive `kek`, and open the sealed DEK.
//!
//! Only the unwrap path requires the private key (and therefore the device +
//! PIN); the seed never leaves the host in cleartext after wrapping.
//!
//! Requires an RSA key pair on the token where both the public and private key
//! objects are visible to PKCS#11 (true for SoftHSM and most HSMs). The runtime
//! path is exercised by the ignored integration test in
//! `tests/pkcs11_device.rs` against a configured module.

use async_trait::async_trait;
use base64::Engine;
use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::mechanism::rsa::{PkcsMgfType, PkcsOaepParams, PkcsOaepSource};
use cryptoki::mechanism::{Mechanism, MechanismType};
use cryptoki::object::{Attribute, ObjectClass, ObjectHandle};
use cryptoki::session::{Session, UserType};
use cryptoki::slot::Slot;
use cryptoki::types::AuthPin;
use hkdf::Hkdf;
use infrazeug_secrets::kek::{open, seal};
use infrazeug_secrets::provider::{Provider, ProviderKind, RecipientEntry, WrapCtx};
use infrazeug_secrets::{Result, SecretsError};
use sha2::Sha256;

const KEK_INFO: &[u8] = b"infrazeug-pkcs11-device-hkdf-v1";
const SEED_LEN: usize = 32;

/// Connection parameters for a PKCS#11 token.
#[derive(Clone)]
pub struct Pkcs11DeviceConfig {
    /// Path to the PKCS#11 module shared object (e.g.
    /// `/usr/lib/softhsm/libsofthsm2.so` or an opensc PKCS#11 module).
    pub module: String,
    /// Token slot id; when `None`, the first slot reporting a token is used.
    pub slot: Option<u64>,
    /// User PIN.
    pub pin: String,
    /// Optional `CKA_ID` selector (raw bytes) to disambiguate the key pair.
    pub key_id: Option<Vec<u8>>,
    /// Optional `CKA_LABEL` selector to disambiguate the key pair.
    pub key_label: Option<String>,
}

impl Pkcs11DeviceConfig {
    pub fn new(module: impl Into<String>, pin: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            slot: None,
            pin: pin.into(),
            key_id: None,
            key_label: None,
        }
    }
}

/// Recipient backed by an on-token RSA key pair.
pub struct Pkcs11Device {
    cfg: Pkcs11DeviceConfig,
}

impl Pkcs11Device {
    pub fn new(cfg: Pkcs11DeviceConfig) -> Self {
        Self { cfg }
    }

    fn err(e: impl std::fmt::Display) -> SecretsError {
        SecretsError::Provider(format!("pkcs11: {e}"))
    }

    /// Open a logged-in session on the configured slot.
    fn session(&self) -> Result<Session> {
        let ctx = Pkcs11::new(&self.cfg.module).map_err(Self::err)?;
        ctx.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
            .map_err(Self::err)?;
        let slot = match self.cfg.slot {
            Some(id) => Slot::try_from(id).map_err(Self::err)?,
            None => *ctx
                .get_slots_with_token()
                .map_err(Self::err)?
                .first()
                .ok_or_else(|| Self::err("no slot with a token present"))?,
        };
        let session = ctx.open_ro_session(slot).map_err(Self::err)?;
        let pin = AuthPin::new(self.cfg.pin.clone().into_boxed_str());
        session
            .login(UserType::User, Some(&pin))
            .map_err(Self::err)?;
        Ok(session)
    }

    /// Selector template common to both key objects (excluding class).
    fn selectors(&self) -> Vec<Attribute> {
        let mut t = Vec::new();
        if let Some(id) = &self.cfg.key_id {
            t.push(Attribute::Id(id.clone()));
        }
        if let Some(label) = &self.cfg.key_label {
            t.push(Attribute::Label(label.clone().into_bytes()));
        }
        t
    }

    fn find_key(&self, session: &Session, class: ObjectClass) -> Result<ObjectHandle> {
        let mut template = vec![Attribute::Class(class)];
        template.extend(self.selectors());
        session
            .find_objects(&template)
            .map_err(Self::err)?
            .into_iter()
            .next()
            .ok_or_else(|| Self::err("matching key object not found on token"))
    }

    fn derive_kek(seed: &[u8], data_key_id: &str) -> [u8; 32] {
        let hk = Hkdf::<Sha256>::new(Some(KEK_INFO), seed);
        let mut okm = [0u8; 32];
        hk.expand(data_key_id.as_bytes(), &mut okm)
            .expect("hkdf expand");
        okm
    }

    fn wrap_mechanism() -> Mechanism<'static> {
        Mechanism::RsaPkcsOaep(PkcsOaepParams::new(
            MechanismType::SHA256,
            PkcsMgfType::MGF1_SHA256,
            PkcsOaepSource::empty(),
        ))
    }
}

#[async_trait]
impl Provider for Pkcs11Device {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Pkcs11
    }

    async fn wrap(&self, dek: &[u8; 32], ctx: &WrapCtx, label: &str) -> Result<RecipientEntry> {
        use rand::RngCore;
        let mut seed = [0u8; SEED_LEN];
        rand::thread_rng().fill_bytes(&mut seed);

        let session = self.session()?;
        let pubkey = self.find_key(&session, ObjectClass::PUBLIC_KEY)?;
        let ciphertext = session
            .encrypt(&Self::wrap_mechanism(), pubkey, &seed)
            .map_err(Self::err)?;

        let kek = Self::derive_kek(&seed, &ctx.data_key_id);
        let wrapped_key = seal(&kek, dek)?;

        Ok(RecipientEntry {
            kind: ProviderKind::Pkcs11,
            label: label.to_string(),
            wrapped_key,
            params: serde_json::json!({
                "device": "pkcs11",
                "mechanism": "rsa-oaep-sha256",
                "seed_ct": base64::engine::general_purpose::STANDARD.encode(&ciphertext),
                "key_label": self.cfg.key_label,
            }),
        })
    }

    async fn unwrap(&self, entry: &RecipientEntry, ctx: &WrapCtx) -> Result<Vec<u8>> {
        let ct_b64 = entry
            .params
            .get("seed_ct")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Self::err("recipient entry missing seed_ct"))?;
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(ct_b64)
            .map_err(Self::err)?;

        let session = self.session()?;
        let privkey = self.find_key(&session, ObjectClass::PRIVATE_KEY)?;
        let seed = session
            .decrypt(&Self::wrap_mechanism(), privkey, &ciphertext)
            .map_err(Self::err)?;

        let kek = Self::derive_kek(&seed, &ctx.data_key_id);
        open(&kek, &entry.wrapped_key)
    }
}
