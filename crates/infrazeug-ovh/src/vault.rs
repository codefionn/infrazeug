//! Standard [`ShellOp::VaultWrite`] nodes that store native capture JSON fields.

use infrazeug_api::builder::InfraBuilder;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_shell::{FileSource, ShellOp};

/// Where generated credentials are stored (`files/mutable/…` via [`ShellOp::mutable_vault_write`]).
#[derive(Clone, Debug)]
pub struct MutableVaultTarget {
    pub data_key_id: String,
    /// Path relative to `mutable/` (e.g. `cloud/backups.vault`).
    pub file: String,
}

impl MutableVaultTarget {
    pub fn new(data_key_id: impl Into<String>, file: impl Into<String>) -> Self {
        Self {
            data_key_id: data_key_id.into(),
            file: file.into(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn vault_field_from_native_capture(
    builder: InfraBuilder,
    node_id: NodeId,
    name: &str,
    machine_id: MachineId,
    vault: &MutableVaultTarget,
    field: &str,
    json_pointer: &str,
    // `optional`: when the capture field may be absent (e.g. a secret returned
    // only at credential creation), skip the write instead of failing the apply.
    optional: bool,
    from_native: NodeId,
    deps: impl IntoIterator<Item = NodeId>,
) -> anyhow::Result<InfraBuilder> {
    let capture = FileSource::capture_same_machine(from_native.0);
    let capture = if optional {
        capture.json_pointer_optional(json_pointer)
    } else {
        capture.json_pointer(json_pointer)
    };
    builder
        .shell_node(
            node_id,
            machine_id,
            ShellOp::mutable_vault_write(&vault.data_key_id, &vault.file, field, capture),
        )
        .name(name)
        .deps(deps)
        .on_upstream_change()
        .build()
}
