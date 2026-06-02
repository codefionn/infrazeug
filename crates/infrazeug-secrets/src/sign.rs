use crate::error::{Result, SecretsError};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanSignature {
    pub signer_id: String,
    pub public_key: [u8; 32],
    pub signature: Vec<u8>,
}

pub fn sign_digest(digest: &[u8; 32], signing_key: &SigningKey, signer_id: &str) -> PlanSignature {
    let sig: Signature = signing_key.sign(digest);
    PlanSignature {
        signer_id: signer_id.to_string(),
        public_key: signing_key.verifying_key().to_bytes(),
        signature: sig.to_bytes().to_vec(),
    }
}

pub fn verify_signature(digest: &[u8; 32], sig: &PlanSignature) -> Result<()> {
    let vk = VerifyingKey::from_bytes(&sig.public_key).map_err(|_| SecretsError::BadSignature)?;
    let signature =
        Signature::from_slice(&sig.signature).map_err(|_| SecretsError::BadSignature)?;
    vk.verify(digest, &signature)
        .map_err(|_| SecretsError::BadSignature)
}

pub fn generate_signing_key() -> SigningKey {
    let mut seed = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut seed);
    SigningKey::from_bytes(&seed)
}

pub fn signing_key_from_seed(seed: &[u8; 32]) -> SigningKey {
    SigningKey::from_bytes(seed)
}

/// Ed25519 verifying (public) key bytes for a signing seed. Useful for deriving
/// the trusted-signer key that pairs with a signing seed.
pub fn verifying_key_from_seed(seed: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(seed).verifying_key().to_bytes()
}

pub fn field_ed25519_seed(map: &serde_cbor::Value) -> Result<[u8; 32]> {
    let Value::Map(m) = map else {
        return Err(SecretsError::Format("expected map".into()));
    };
    let key = m
        .iter()
        .find(|(k, _)| matches!(k, Value::Text(s) if s == "ed25519_seed"))
        .map(|(_, v)| v)
        .ok_or(SecretsError::MissingField {
            field: "ed25519_seed".into(),
            files: vec![],
        })?;
    match key {
        Value::Bytes(b) if b.len() == 32 => {
            let mut s = [0u8; 32];
            s.copy_from_slice(b);
            Ok(s)
        }
        _ => Err(SecretsError::Format("ed25519_seed must be 32 bytes".into())),
    }
}

use serde_cbor::Value;
