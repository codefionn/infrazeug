//! ShellOp tier-2 DSL (SOUL §3.3).
//!
//! Most infra nodes lower to a serializable [`ShellOp`] tree: file writes,
//! command runs, captures, and become/sudo wrapping. Ops are CBOR-encoded for
//! the push agent and interpreted locally for [`LocalShellExecutor`]; in
//! agentless mode they lower to `ssh`/`sftp` via [`lower`].
//!
//! # Building ops
//!
//! Use the [`argv!`] macro for explicit argument vectors (locked: no shell
//! string parsing). Compose with [`ShellOp::run`], [`ShellOp::write_file`], etc.
//!
//! ```ignore
//! use infrazeug_shell::{argv, FileSource, ShellOp};
//!
//! let op = ShellOp::write_file("/etc/app.conf", FileSource::bytes(b"ok\n"))
//!     .then(ShellOp::run(argv!["systemctl", "reload", "app"]));
//! ```
//!
//! [`resolve_shell_op`] resolves `${capture:…}` placeholders at apply time.
//! Extension crates (`infrazeug-kubectl`, `infrazeug-helm`, …) return `ShellOp`
//! values so the same nodes work in agentless SSH and agent push modes.

pub mod error;
pub mod local;
pub mod lower;
pub mod op;
pub mod resolve;
pub mod source;
pub mod sync_dir;

pub use error::{Result, ShellError};
pub use local::{LocalShellExecutor, OutputChunk, OutputStream};
pub use lower::{lower, Lowered};
pub use op::{EnvVarSource, ShellOp, SyncDirOptions};
pub use resolve::{resolve_shell_op, CaptureLookup};
pub use source::{
    capture_refs, CaptureRef, FileSource, FileSourceTransform, PasswordHashAlgorithm,
    PasswordHashSpec, RandomPasswordSpec, CAPTURE_MAX_BYTES,
};
pub use sync_dir::{pack_sync_plan, plan_sync_dir, sync_dir_to_local, SyncDirEntry, SyncDirPlan};

#[macro_export]
macro_rules! argv {
    ($($piece:expr),* $(,)?) => {
        vec![$($piece.to_string()),*]
    };
}

pub use argv as argv_macro;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cbor_roundtrip() {
        let op = ShellOp::run(argv!["true"]);
        let bytes = serde_cbor::to_vec(&op).unwrap();
        let back: ShellOp = serde_cbor::from_slice(&bytes).unwrap();
        assert_eq!(op, back);
    }
}
