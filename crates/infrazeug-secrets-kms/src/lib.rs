//! age and cloud KMS recipient providers (SOUL §6.3).
//!
//! - [`AgeProvider`] — age X25519 identities as vault recipients (file or env).
//! - [`EnvKmsProvider`] — stub KMS path using `INFRZEUG_KMS_SEED` for tests;
//!   [`KmsConfig`] holds hooks for future cloud KMS backends.
//!
//! Register providers on the [`VaultStore`](infrazeug_secrets::VaultStore) alongside
//! passphrase, ssh-agent, and hardware recipients.

mod age_provider;
mod kms;

pub use age_provider::AgeProvider;
pub use kms::{EnvKmsProvider, KmsConfig};
