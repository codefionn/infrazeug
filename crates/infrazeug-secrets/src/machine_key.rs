//! Per-machine X25519 keys for sealed pull-mode plans (SOUL §3.11.3).
//!
//! # Sealed plan binary microarchitecture
//!
//! Sealed blobs use X25519 ephemeral key exchange with HKDF-SHA256 key
//! derivation and XChaCha20-Poly1305 AEAD encryption:
//!
//! ```text
//!   INFRZSLD (8) │ version (1) │ eph_pub (32) │ nonce (24) │ ciphertext+tag
//! ```
//!
//! Key derivation: `HKDF-SHA256(salt="infrazeug-sealed-plan-v1", ikm=shared_secret,
//! info=eph_pub || recipient_pub)`. Binding both public keys into `info` follows
//! the sealed-box construction and domain-separates each exchange; the X25519
//! shared secret is rejected if non-contributory (all-zero, from a low-order
//! point). See `docs/protocol.md` for the full microarchitecture.

use crate::error::{Result, SecretsError};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use std::io::Write;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, Zeroizing};

pub const SEALED_MAGIC: &[u8; 8] = b"INFRZSLD";
/// v2 binds both public keys into the KDF `info` and rejects non-contributory
/// shared secrets. v1 blobs (recipient_pub-only `info`) are no longer accepted;
/// sealed plans are short-lived and simply republished.
pub const SEALED_VERSION: u8 = 0x02;

#[derive(Clone)]
pub struct MachineKeyPair {
    pub public: [u8; 32],
    secret: Zeroizing<[u8; 32]>,
}

impl MachineKeyPair {
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(rand::thread_rng());
        let public = PublicKey::from(&secret);
        Self {
            public: public.to_bytes(),
            secret: Zeroizing::new(secret.to_bytes()),
        }
    }

    pub fn from_secret_bytes(secret: [u8; 32]) -> Result<Self> {
        let sk = StaticSecret::from(secret);
        let public = PublicKey::from(&sk);
        Ok(Self {
            public: public.to_bytes(),
            secret: Zeroizing::new(secret),
        })
    }

    pub fn secret_bytes(&self) -> &[u8; 32] {
        &self.secret
    }

    pub fn write_private_file(&self, path: &std::path::Path) -> Result<()> {
        #[cfg(unix)]
        let mut f = {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(path)
                .map_err(|e| SecretsError::Io(e.to_string()))?
        };
        #[cfg(not(unix))]
        let mut f = std::fs::File::create(path).map_err(|e| SecretsError::Io(e.to_string()))?;
        f.write_all(self.secret.as_ref())
            .map_err(|e| SecretsError::Io(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| SecretsError::Io(e.to_string()))?;
        }
        Ok(())
    }

    pub fn read_private_file(path: &std::path::Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(|e| SecretsError::Io(e.to_string()))?;
        if bytes.len() != 32 {
            return Err(SecretsError::Format("machine key must be 32 bytes".into()));
        }
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&bytes);
        Self::from_secret_bytes(secret)
    }
}

pub fn seal_bytes(plaintext: &[u8], recipient_public: &[u8; 32]) -> Result<Vec<u8>> {
    let recipient = PublicKey::from(*recipient_public);
    let ephemeral = StaticSecret::random_from_rng(rand::thread_rng());
    let eph_pub = PublicKey::from(&ephemeral).to_bytes();
    let shared = ephemeral.diffie_hellman(&recipient);
    if !is_contributory(&shared) {
        return Err(SecretsError::Encrypt);
    }
    let dek = derive_dek(shared.as_bytes(), &eph_pub, recipient_public);

    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&dek));
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| SecretsError::Encrypt)?;

    let mut out = Vec::new();
    out.extend_from_slice(SEALED_MAGIC);
    out.push(SEALED_VERSION);
    out.extend_from_slice(&eph_pub);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn unseal_bytes(blob: &[u8], recipient_secret: &[u8; 32]) -> Result<Vec<u8>> {
    if blob.len() < 8 + 1 + 32 + 24 + 16 {
        return Err(SecretsError::Format("sealed blob too short".into()));
    }
    if &blob[0..8] != SEALED_MAGIC {
        return Err(SecretsError::Format("bad sealed magic".into()));
    }
    if blob[8] != SEALED_VERSION {
        return Err(SecretsError::Format("bad sealed version".into()));
    }
    let mut eph = [0u8; 32];
    eph.copy_from_slice(&blob[9..41]);
    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(&blob[41..65]);
    let ct = &blob[65..];

    let sk = StaticSecret::from(*recipient_secret);
    let eph_pk = PublicKey::from(eph);
    let shared = sk.diffie_hellman(&eph_pk);
    if !is_contributory(&shared) {
        return Err(SecretsError::Decrypt);
    }
    let recipient_pub = PublicKey::from(&sk).to_bytes();
    let dek = derive_dek(shared.as_bytes(), &eph, &recipient_pub);

    let cipher = XChaCha20Poly1305::new(Key::from_slice(&dek));
    cipher
        .decrypt(XNonce::from_slice(&nonce), ct)
        .map_err(|_| SecretsError::Decrypt)
}

/// Reject a non-contributory X25519 exchange: a low-order `eph_pub` drives the
/// shared secret to all-zero, which would make the derived key attacker-known.
fn is_contributory(shared: &x25519_dalek::SharedSecret) -> bool {
    shared.as_bytes().iter().any(|&b| b != 0)
}

fn derive_dek(shared: &[u8], eph_public: &[u8; 32], recipient_public: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(b"infrazeug-sealed-plan-v1"), shared);
    let mut info = [0u8; 64];
    info[..32].copy_from_slice(eph_public);
    info[32..].copy_from_slice(recipient_public);
    let mut okm = [0u8; 32];
    hk.expand(&info, &mut okm).expect("hkdf expand");
    okm
}

impl Drop for MachineKeyPair {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_roundtrip() {
        let pair = MachineKeyPair::generate();
        let plain = b"plan-bytes";
        let sealed = seal_bytes(plain, &pair.public).unwrap();
        let out = unseal_bytes(&sealed, pair.secret_bytes()).unwrap();
        assert_eq!(out, plain);
    }

    #[test]
    fn unseal_rejects_old_version() {
        let pair = MachineKeyPair::generate();
        let mut sealed = seal_bytes(b"x", &pair.public).unwrap();
        // Downgrade the version byte: the new KDF binds both pubkeys, so v1
        // blobs must be rejected rather than decrypted under the wrong scheme.
        sealed[8] = 0x01;
        assert!(unseal_bytes(&sealed, pair.secret_bytes()).is_err());
    }

    #[test]
    fn unseal_rejects_tampered_ephemeral_pub() {
        let pair = MachineKeyPair::generate();
        let mut sealed = seal_bytes(b"x", &pair.public).unwrap();
        // Flip a byte of the ephemeral public key; AEAD must fail to open.
        sealed[9] ^= 0xff;
        assert!(unseal_bytes(&sealed, pair.secret_bytes()).is_err());
    }
}
