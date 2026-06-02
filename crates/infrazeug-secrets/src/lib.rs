//! Encrypted vault store (SOUL §6).
//!
//! Secrets use a two-tier model: **recipients** (passphrase, ssh-agent, FIDO2,
//! PKCS#11, age, KMS, …) wrap **DataKeys**, which encrypt individual vault files.
//! On disk: CBOR headers + XChaCha20-Poly1305 payloads; plan signing uses Ed25519
//! keys also stored in the vault.
//!
//! # Core types
//!
//! - [`VaultStore`] — read/write encrypted files against a [`Backend`].
//! - [`Provider`] / [`PassphraseProvider`] — wrap and unwrap DEKs at unlock time.
//! - [`MultiBackend`] — replicate objects across S3, WebDAV, filesystem, etc.
//! - [`VaultRef`] — typed `vault://` paths resolved during apply (never via MCP).
//! - [`machine_key`] — X25519 keys for sealed pull-mode plan blobs.
//!
//! Provider and backend implementations live in sibling crates
//! (`infrazeug-secrets-ssh`, `-hw`, `-kms`, `-s3`, `-dav`). On-disk layout is
//! documented in `docs/vault-format.md` in the repo root.

mod audit;
pub mod backend;
mod envelope;
mod error;
mod format;
pub mod kek;
mod layered;
pub mod machine_key;
mod migrate;
mod multi;
pub mod provider;
mod sign;
mod store;
mod store_format;
mod vault_ref;

pub use audit::{append_audit, AuditEntry};
pub use backend::{Backend, Etag, FsBackend, ObjectMeta};
pub use envelope::{
    create_envelope, decode_dkey_blob, deduplicate_recipient_labels, encode_dkey_blob,
    envelope_cbor, envelope_from_cbor, generate_dek, parse_dkey_blob, unlock_envelope,
    DataKeyEnvelope,
};
pub use error::{Result, SecretsError};
pub use format::{
    collect_vault_field_paths, decrypt_map, encrypt_map, field_from_map, vault_header_from_blob,
    VaultHeader, MAGIC, VERSION,
};
pub use layered::load_layered;
pub use machine_key::{seal_bytes, unseal_bytes, MachineKeyPair, SEALED_MAGIC};
pub use migrate::ensure_store_format;
pub use multi::{MultiBackend, ReadPolicy, WritePolicy};
pub use provider::{PassphraseProvider, Provider, ProviderKind, RecipientEntry, WrapCtx};
pub use sign::{
    field_ed25519_seed, generate_signing_key, sign_digest, signing_key_from_seed, verify_signature,
    verifying_key_from_seed, PlanSignature,
};
pub use store::{VaultFileKey, VaultStore};
pub use store_format::{
    StoreMeta, DKEY_MAGIC, DKEY_WIRE_VERSION, META_KEY, STORE_FORMAT_VERSION, VAULT_FILE_VERSION,
};
pub use vault_ref::{mutable_vault_path, VaultRef, MUTABLE_VAULT_PREFIX};

#[cfg(test)]
mod store_tests;
