//! Shared KEK seal/open for challenge-derived providers (SOUL §6.3).

use crate::error::{Result, SecretsError};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngCore;

const CHALLENGE_PREFIX: &[u8] = b"infrazeug-kek-v1\0";

pub fn challenge(data_key_id: &str, file_salt: &[u8; 32]) -> Vec<u8> {
    let mut c = Vec::new();
    c.extend_from_slice(CHALLENGE_PREFIX);
    c.extend_from_slice(data_key_id.as_bytes());
    c.extend_from_slice(file_salt);
    c
}

/// Seal the DEK under the KEK, binding it to its envelope context (`aad`,
/// normally [`challenge`] over `data_key_id || file_salt`) so a `wrapped_key`
/// cannot be lifted into a different envelope without the tag failing.
pub fn seal(kek: &[u8; 32], dek: &[u8; 32], aad: &[u8]) -> Result<Vec<u8>> {
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

/// Open a [`seal`]ed blob. Falls back to verifying without AAD for keys sealed
/// before context binding existed (legacy recipients keep unlocking; new seals
/// are always bound).
pub fn open(kek: &[u8; 32], blob: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < 24 {
        return Err(SecretsError::Provider("wrapped dek short".into()));
    }
    let (nonce, ct) = blob.split_at(24);
    let key = Key::from_slice(kek);
    let cipher = XChaCha20Poly1305::new(key);
    let bound = cipher.decrypt(
        XNonce::from_slice(nonce),
        chacha20poly1305::aead::Payload { msg: ct, aad },
    );
    bound.or_else(|_| {
        cipher
            .decrypt(XNonce::from_slice(nonce), ct)
            .map_err(|_| SecretsError::Decrypt)
    })
}
