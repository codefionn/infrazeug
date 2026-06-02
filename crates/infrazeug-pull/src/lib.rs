//! Pull-mode plan store, sealed slices, and on-host apply (SOUL §3.11).
//!
//! # Pull-mode microarchitecture
//!
//! In pull mode the target host fetches its own sealed [`PlanSlice`] from
//! a shared [`PlanStore`] (filesystem, S3, etc.), unseals it with its
//! X25519 private key, verifies Ed25519 signatures, and applies locally
//! without any live controller connection.
//!
//! ```text
//!   Controller                        PlanStore                Target Host
//!   plan-op publish ──sealed blob──> plans/{id}.plan.sealed ──unseal──> apply
//!   machine register ──pubkey──────> machines/{id}.pub
//! ```
//!
//! Cross-machine dependencies are **forbidden** in pull slices
//! (`SliceMode::Pull` returns `PullSliceNeedsWait`). See `docs/protocol.md`
//! for the full microarchitecture.

mod bootstrap;
mod daemon;
mod error;
mod fetch_auth;
mod mode;
mod publish;
mod serve;
mod store;

pub use bootstrap::{parse_bootstrap, Bootstrap};
pub use daemon::{
    open_fs_store, parse_trusted_signers, run_daemon, run_from_bootstrap, run_oneshot,
};
pub use error::{PullError, Result};
pub use fetch_auth::FetchAuth;
pub use mode::PullMode;
pub use publish::{
    machine_keygen, publish_slice, register_machine_pubkey, revoke_machine, PublishOptions,
};
pub use serve::apply_sealed_slice;
pub use store::PlanStore;

#[cfg(test)]
mod lib_tests;
