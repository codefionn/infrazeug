//! ssh-agent recipient provider (SOUL §6.3) via `SSH_AUTH_SOCK`.
//!
//! [`SshAgentProvider`] uses the running agent to wrap/unwrap DataKeys without
//! storing long-term key material in the vault. Requires `SSH_AUTH_SOCK` and a
//! loaded key that matches a recipient entry created at `infrazeug vault` time.

mod agent;

use agent::ssh_agent_sign;
use async_trait::async_trait;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use infrazeug_secrets::provider::{Provider, ProviderKind, RecipientEntry, WrapCtx};
use infrazeug_secrets::{Result, SecretsError};
use rand::RngCore;
use sha2::Sha256;

const KEK_INFO: &[u8] = b"infrazeug-kek-hkdf-v1";
const CHALLENGE_PREFIX: &[u8] = b"infrazeug-kek-v1\0";

pub struct SshAgentProvider {
    pub key_comment: String,
}

impl SshAgentProvider {
    pub fn new(key_comment: impl Into<String>) -> Self {
        Self {
            key_comment: key_comment.into(),
        }
    }

    fn challenge(ctx: &WrapCtx) -> Vec<u8> {
        let mut c = Vec::new();
        c.extend_from_slice(CHALLENGE_PREFIX);
        c.extend_from_slice(ctx.data_key_id.as_bytes());
        c.extend_from_slice(&ctx.file_salt);
        c
    }

    fn derive_kek(sig: &[u8], data_key_id: &str) -> [u8; 32] {
        let hk = Hkdf::<Sha256>::new(Some(KEK_INFO), sig);
        let mut okm = [0u8; 32];
        hk.expand(data_key_id.as_bytes(), &mut okm)
            .expect("hkdf expand");
        okm
    }

    async fn sign_challenge(&self, challenge: &[u8]) -> Result<Vec<u8>> {
        ssh_agent_sign(&self.key_comment, challenge).await
    }

    fn seal(kek: &[u8; 32], dek: &[u8; 32]) -> Result<Vec<u8>> {
        let mut nonce = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce);
        let key = Key::from_slice(kek);
        let cipher = XChaCha20Poly1305::new(key);
        let ct = cipher
            .encrypt(XNonce::from_slice(&nonce), dek.as_ref())
            .map_err(|_| SecretsError::Decrypt)?;
        let mut out = nonce.to_vec();
        out.extend(ct);
        Ok(out)
    }

    fn open(kek: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>> {
        if blob.len() < 24 {
            return Err(SecretsError::Provider("wrapped dek short".into()));
        }
        let (nonce, ct) = blob.split_at(24);
        let key = Key::from_slice(kek);
        let cipher = XChaCha20Poly1305::new(key);
        cipher
            .decrypt(XNonce::from_slice(nonce), ct)
            .map_err(|_| SecretsError::Decrypt)
    }
}

#[async_trait]
impl Provider for SshAgentProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::SshAgent
    }

    async fn wrap(&self, dek: &[u8; 32], ctx: &WrapCtx, label: &str) -> Result<RecipientEntry> {
        let challenge = Self::challenge(ctx);
        let sig = self.sign_challenge(&challenge).await?;
        let kek = Self::derive_kek(&sig, &ctx.data_key_id);
        let wrapped_key = Self::seal(&kek, dek)?;
        Ok(RecipientEntry {
            kind: ProviderKind::SshAgent,
            label: label.to_string(),
            wrapped_key,
            params: serde_json::json!({ "comment": self.key_comment }),
        })
    }

    async fn unwrap(&self, entry: &RecipientEntry, ctx: &WrapCtx) -> Result<Vec<u8>> {
        let challenge = Self::challenge(ctx);
        let sig = self.sign_challenge(&challenge).await?;
        let kek = Self::derive_kek(&sig, &ctx.data_key_id);
        Self::open(&kek, &entry.wrapped_key)
    }
}
