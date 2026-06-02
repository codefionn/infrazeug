use crate::error::{Result, SecretsError};
use crate::provider::{Provider, ProviderKind, RecipientEntry, WrapCtx};
use crate::store_format::{DKEY_MAGIC, DKEY_WIRE_VERSION};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataKeyEnvelope {
    pub id: String,
    pub file_salt: [u8; 32],
    pub recipients: Vec<RecipientEntry>,
    /// MAC over the whole envelope (recipient set, params, wrapped keys) keyed
    /// by a DEK-derived key: `nonce(24) || tag(16)`. Detects tampering or
    /// recipient stripping/reordering by anyone without the unlocked DEK.
    /// Empty only for legacy envelopes written before authentication existed.
    #[serde(default)]
    pub auth: Vec<u8>,
}

const ENVELOPE_AUTH_INFO: &[u8] = b"infrazeug-envelope-auth-v1";

fn envelope_auth_key(dek: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(ENVELOPE_AUTH_INFO);
    h.update(dek);
    h.finalize().into()
}

/// Canonical bytes the auth tag covers: the envelope with `auth` cleared.
fn envelope_auth_aad(envelope: &DataKeyEnvelope) -> Result<Vec<u8>> {
    let mut probe = envelope.clone();
    probe.auth = Vec::new();
    Ok(serde_cbor::to_vec(&probe)?)
}

/// Compute and store the envelope authentication tag. Call on every mutation
/// before persisting; requires the unlocked DEK.
pub fn seal_envelope_auth(envelope: &mut DataKeyEnvelope, dek: &[u8; 32]) -> Result<()> {
    envelope.auth = Vec::new();
    let aad = envelope_auth_aad(envelope)?;
    let key = envelope_auth_key(dek);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    let tag = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &[],
                aad: &aad,
            },
        )
        .map_err(|_| SecretsError::Encrypt)?;
    let mut out = nonce.to_vec();
    out.extend(tag);
    envelope.auth = out;
    Ok(())
}

/// Verify the envelope authentication tag against the unlocked DEK. Rejects
/// tampered or unauthenticated envelopes. Legacy envelopes with an empty `auth`
/// skip verification until [`crate::migrate::migrate_envelope_after_unlock`].
pub fn verify_envelope_auth(envelope: &DataKeyEnvelope, dek: &[u8; 32]) -> Result<()> {
    if envelope.auth.is_empty() {
        return Ok(());
    }
    if envelope.auth.len() < 24 + 16 {
        return Err(SecretsError::BadSignature);
    }
    let (nonce, tag) = envelope.auth.split_at(24);
    let aad = envelope_auth_aad(envelope)?;
    let key = envelope_auth_key(dek);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: tag,
                aad: &aad,
            },
        )
        .map(|_| ())
        .map_err(|_| SecretsError::BadSignature)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataKeyFile {
    pub envelope: DataKeyEnvelope,
}

pub fn generate_dek() -> [u8; 32] {
    let mut dek = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut dek);
    dek
}

pub async fn create_envelope(
    id: &str,
    dek: &[u8; 32],
    provider: &dyn Provider,
    label: &str,
) -> Result<DataKeyEnvelope> {
    let mut file_salt = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut file_salt);
    let ctx = WrapCtx {
        data_key_id: id.to_string(),
        file_salt,
    };
    let entry = provider.wrap(dek, &ctx, label).await?;
    let mut envelope = DataKeyEnvelope {
        id: id.to_string(),
        file_salt,
        recipients: vec![entry],
        auth: Vec::new(),
    };
    seal_envelope_auth(&mut envelope, dek)?;
    Ok(envelope)
}

pub async fn unlock_envelope(
    envelope: &DataKeyEnvelope,
    provider: &dyn Provider,
    entry: &RecipientEntry,
) -> Result<Zeroizing<[u8; 32]>> {
    let ctx = WrapCtx {
        data_key_id: envelope.id.clone(),
        file_salt: envelope.file_salt,
    };
    // Wrap the provider's returned key material so the cleartext DEK copy is
    // zeroized on drop rather than left in freed heap.
    let dek_bytes = Zeroizing::new(provider.unwrap(entry, &ctx).await?);
    if dek_bytes.len() != 32 {
        return Err(SecretsError::Provider("bad dek length".into()));
    }
    let mut dek = [0u8; 32];
    dek.copy_from_slice(&dek_bytes);
    Ok(Zeroizing::new(dek))
}

pub fn envelope_cbor(envelope: &DataKeyEnvelope) -> Result<Vec<u8>> {
    encode_dkey_blob(envelope)
}

pub fn envelope_from_cbor(bytes: &[u8]) -> Result<DataKeyEnvelope> {
    decode_dkey_blob(bytes)
}

/// Encode `keys/<id>.dkey` (current wire format: `INFRZDKEY` + CBOR body).
pub fn encode_dkey_blob(envelope: &DataKeyEnvelope) -> Result<Vec<u8>> {
    let inner = serde_cbor::to_vec(&DataKeyFile {
        envelope: envelope.clone(),
    })?;
    let mut out = Vec::with_capacity(8 + 1 + 4 + inner.len());
    out.extend_from_slice(DKEY_MAGIC);
    out.push(DKEY_WIRE_VERSION);
    out.extend_from_slice(&(inner.len() as u32).to_be_bytes());
    out.extend_from_slice(&inner);
    Ok(out)
}

/// Decode a data-key blob (wrapped v1 or legacy bare CBOR) without label checks.
pub fn parse_dkey_blob(bytes: &[u8]) -> Result<DataKeyEnvelope> {
    let inner = dkey_inner_bytes(bytes)?;
    let f: DataKeyFile =
        serde_cbor::from_slice(inner).map_err(|e| SecretsError::Format(e.to_string()))?;
    Ok(f.envelope)
}

/// Decode a data-key blob and require unique recipient labels.
pub fn decode_dkey_blob(bytes: &[u8]) -> Result<DataKeyEnvelope> {
    let envelope = parse_dkey_blob(bytes)?;
    validate_unique_recipient_labels(&envelope)?;
    Ok(envelope)
}

fn dkey_inner_bytes(bytes: &[u8]) -> Result<&[u8]> {
    if bytes.len() >= 8 && &bytes[0..8] == DKEY_MAGIC {
        if bytes.len() < 13 {
            return Err(SecretsError::Format("truncated dkey wire header".into()));
        }
        if bytes[8] != DKEY_WIRE_VERSION {
            return Err(SecretsError::Format(format!(
                "unsupported dkey wire version {}",
                bytes[8]
            )));
        }
        let hlen = u32::from_be_bytes(bytes[9..13].try_into().unwrap()) as usize;
        if bytes.len() < 13 + hlen {
            return Err(SecretsError::Format("truncated dkey wire body".into()));
        }
        Ok(&bytes[13..13 + hlen])
    } else {
        Ok(bytes)
    }
}

/// Drop later recipients that reuse a label (keeps the first). Returns `true` if any removed.
pub fn deduplicate_recipient_labels(envelope: &mut DataKeyEnvelope) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut removed = false;
    envelope.recipients.retain(|r| {
        if seen.insert(r.label.clone()) {
            true
        } else {
            removed = true;
            false
        }
    });
    removed
}

/// Recipient labels must be unique within a data key (used for unlock and CLI UX).
pub fn validate_unique_recipient_labels(envelope: &DataKeyEnvelope) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for r in &envelope.recipients {
        if !seen.insert(r.label.as_str()) {
            return Err(SecretsError::Format(format!(
                "duplicate recipient label {:?} in data key {}",
                r.label, envelope.id
            )));
        }
    }
    Ok(())
}

pub fn find_recipient<'a>(
    envelope: &'a DataKeyEnvelope,
    kind: ProviderKind,
    label: &str,
) -> Option<&'a RecipientEntry> {
    envelope
        .recipients
        .iter()
        .find(|r| r.kind == kind && r.label == label)
}
