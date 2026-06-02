//! Run-scoped input access handed to native methods through the node context.

use crate::error::Result;
use async_trait::async_trait;
use uuid::Uuid;

/// Read-only access to controller-side run inputs during native execution.
///
/// Injected into [`NodeCtx`](crate::NodeCtx) / [`PlanCtx`](crate::PlanCtx) for
/// `Local` (controller) native nodes so a method can read credentials from the
/// vault, generated mutable-vault values, and upstream node captures. Remote
/// agents never receive it — the controller vault and capture store stay on the
/// controller.
///
/// The trait lives in `infrazeug-native` (the lowest tier) so the context types can
/// carry it without depending on `infrazeug-secrets`; `infrazeug-core` provides the
/// concrete vault-backed implementation.
#[async_trait]
pub trait SecretSource: Send + Sync {
    /// Whether this source can read regular vault fields.
    fn has_vault(&self) -> bool {
        true
    }

    /// Resolve a vault field (`file`, dot-path `field`) to its raw value bytes.
    async fn read_field(&self, file: &str, field: &str) -> Result<Vec<u8>>;

    /// Whether this source can read generated mutable-vault fields.
    fn has_mutable_vault(&self) -> bool {
        self.has_vault()
    }

    /// Resolve a generated mutable-vault field (`files/mutable/{file}`).
    async fn read_mutable_field(&self, file: &str, field: &str) -> Result<Vec<u8>> {
        let _ = (file, field);
        Err(crate::NativeError::other(
            "mutable vault source unavailable on this context",
        ))
    }

    /// Whether this source can read upstream node captures.
    fn has_node_captures(&self) -> bool {
        false
    }

    /// Resolve a completed upstream node capture on `machine`.
    async fn read_node_capture(&self, node: Uuid, machine: Uuid) -> Result<Vec<u8>> {
        let _ = (node, machine);
        Err(crate::NativeError::other(
            "node capture source unavailable on this context",
        ))
    }
}
