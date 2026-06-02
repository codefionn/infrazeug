//! Interactive SSH auth resolution seam.
//!
//! The transport factory authenticates the first connection to a machine, but
//! the means to obtain a secret (prompt the operator, or read the controller
//! vault) live in higher layers. [`SshAuthResolver`] is the seam: the factory
//! asks it for a machine's askpass secret file on demand, so lazy and
//! dynamically-discovered machines — whose [`SshConfig`] is only known mid-run —
//! authenticate the same way as statically-declared ones.

use async_trait::async_trait;
use infrazeug_core::id::MachineId;
use infrazeug_core::machine::SshConfig;
use std::path::PathBuf;

/// Resolves a machine's interactive SSH secret into an askpass file on demand.
///
/// Implementations cache per machine (reconnects must not re-prompt) and
/// serialize resolution (at most one prompt outstanding at a time).
#[async_trait]
pub trait SshAuthResolver: Send + Sync {
    /// The askpass secret file for `machine`'s interactive auth, resolving and
    /// caching it on first call. Returns `Ok(None)` for non-interactive machines.
    async fn askpass_file(
        &self,
        machine: MachineId,
        ssh: &SshConfig,
    ) -> std::result::Result<Option<PathBuf>, String>;
}
