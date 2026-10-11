//! Node stdout captures for cross-node / cross-machine `WriteFile` (SOUL §3.3.3).

use crate::error::{CoreError, Result};
use crate::id::{MachineId, NodeId};
use infrazeug_shell::{capture_refs, CAPTURE_MAX_BYTES};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Write bytes to a new `0600` file via an `O_EXCL` temp file + rename, so
/// pre-planted symlinks are replaced rather than followed (see
/// [`crate::ssh_askpass::write_secret_file`]).
async fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let dir = path
        .parent()
        .ok_or_else(|| CoreError::other("spill path has no parent directory"))?;
    let tmp = dir.join(format!(".spill-{}", uuid::Uuid::new_v4()));
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(CoreError::from)?;
    std::io::Write::write_all(&mut f, bytes).map_err(CoreError::from)?;
    f.sync_all().map_err(CoreError::from)?;
    drop(f);
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(CoreError::from(e));
    }
    Ok(())
}

#[derive(Clone, Default)]
pub struct CaptureStore {
    inner: Arc<Mutex<CaptureInner>>,
}

#[derive(Default)]
struct CaptureInner {
    memory: HashMap<(NodeId, MachineId), Vec<u8>>,
    spill: HashMap<(NodeId, MachineId), PathBuf>,
}

impl CaptureStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn put(
        &self,
        node: NodeId,
        machine: MachineId,
        stdout: Vec<u8>,
        spill_root: Option<&PathBuf>,
    ) -> Result<()> {
        if stdout.len() > CAPTURE_MAX_BYTES {
            if let Some(root) = spill_root {
                let rel = format!("{}/{}/stdout", node.0, machine.0);
                let path = root.join(rel);
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                // Captures can carry secret material: spill owner-only.
                write_private(&path, &stdout).await?;
                let mut g = self.inner.lock().await;
                g.spill.insert((node, machine), path);
                return Ok(());
            }
            return Err(CoreError::CaptureTooLarge {
                node: node.0.to_string(),
                machine: machine.0.to_string(),
                bytes: stdout.len(),
                limit: CAPTURE_MAX_BYTES,
            });
        }
        let mut g = self.inner.lock().await;
        g.memory.insert((node, machine), stdout);
        Ok(())
    }

    pub async fn get(&self, node: NodeId, machine: MachineId) -> Result<Vec<u8>> {
        let g = self.inner.lock().await;
        if let Some(bytes) = g.memory.get(&(node, machine)) {
            return Ok(bytes.clone());
        }
        if let Some(path) = g.spill.get(&(node, machine)) {
            let bytes = tokio::fs::read(path).await?;
            return Ok(bytes);
        }
        Err(CoreError::CaptureMissing {
            node: node.0.to_string(),
            machine: machine.0.to_string(),
        })
    }

    pub async fn lookup_map(&self) -> Result<HashMap<(uuid::Uuid, uuid::Uuid), Vec<u8>>> {
        let g = self.inner.lock().await;
        let mut map = HashMap::new();
        for ((node, machine), bytes) in &g.memory {
            map.insert((node.0, machine.0), bytes.clone());
        }
        for ((node, machine), path) in &g.spill {
            let bytes = tokio::fs::read(path).await?;
            map.insert((node.0, machine.0), bytes);
        }
        Ok(map)
    }
}

/// Resolve capture references in `op` using the async store.
pub async fn resolve_op_captures(
    op: &infrazeug_shell::ShellOp,
    on_machine: MachineId,
    store: &CaptureStore,
) -> infrazeug_shell::Result<infrazeug_shell::ShellOp> {
    let map = store
        .lookup_map()
        .await
        .map_err(|e| infrazeug_shell::ShellError::Other(e.to_string()))?;
    infrazeug_shell::resolve_shell_op(op, on_machine.0, &map)
}

/// True when the captured stdout for `(node, machine)` is destined for the
/// vault — its bytes are consumed by a [`ShellOp::VaultWrite`] value somewhere
/// in `infra` and are therefore secret material that must never leave the
/// process in tool output (SOUL: MCP tools never expose secrets).
///
/// Matches both static nodes and dynamic-group template instances (whose
/// capture keys are [`dyn_instance_node_id`](crate::dynamic::dyn_instance_node_id)
/// remaps of the template node ids).
pub fn capture_is_secret(
    infra: &crate::infra::Infra,
    node: uuid::Uuid,
    machine: uuid::Uuid,
) -> bool {
    let is_secret_ref = |reference: &infrazeug_shell::CaptureRef| -> bool {
        if let Some(from) = reference.machine {
            if from != machine {
                return false;
            }
        }
        if reference.node == node {
            return true;
        }
        // Dynamic-group template instance: the capture key is the template
        // node id remapped onto the executing machine.
        crate::dynamic::dyn_instance_node_id(NodeId(reference.node), MachineId(machine)).0 == node
    };
    let mut stack: Vec<&crate::node::Node> = infra.nodes.iter().collect();
    for group in &infra.dynamic_groups {
        stack.extend(group.template.iter());
    }
    for owner in stack {
        let crate::node::NodeBody::Shell(op) = &owner.body else {
            continue;
        };
        if infrazeug_shell::secret_capture_refs(op).iter().any(&is_secret_ref) {
            return true;
        }
    }
    false
}

/// Fail-fast capture validation: returns the first invalid capture reference.
pub fn validate_capture_refs(infra: &crate::infra::Infra) -> Result<()> {
    let mut report = crate::lint::LintReport::new();
    collect_capture_refs(infra, &mut report);
    report.into_result()
}

/// Collect every invalid capture reference into `report` (does not short-circuit).
pub fn collect_capture_refs(infra: &crate::infra::Infra, report: &mut crate::lint::LintReport) {
    let node_by_id: HashMap<NodeId, &crate::node::Node> =
        infra.nodes.iter().map(|n| (n.id, n)).collect();

    for node in &infra.nodes {
        let crate::node::NodeBody::Shell(op) = &node.body else {
            continue;
        };
        for reference in capture_refs(op) {
            let source_node = NodeId(reference.node);
            let Some(source) = node_by_id.get(&source_node) else {
                report.error(
                    CoreError::CaptureUnknownNode {
                        consumer: node.name.clone(),
                        node: reference.node.to_string(),
                    },
                    "the captured-from node id is not in this infra".to_string(),
                );
                continue;
            };
            if !node.deps.contains(&source_node) {
                report.error(
                    CoreError::CaptureNotInDeps {
                        consumer: node.name.clone(),
                        upstream: source.name.clone(),
                    },
                    format!(
                        "add `{}` to `{}`'s deps so the capture is ordered before use",
                        source.name, node.name
                    ),
                );
            }
            if let Some(from) = reference.machine {
                let from_mid = MachineId(from);
                match infra.resolve_targets(&source.targets) {
                    Ok(source_machines) if source_machines.contains(&from_mid) => {}
                    Ok(_) => report.error(
                        CoreError::CaptureInvalidMachine {
                            consumer: node.name.clone(),
                            upstream: source.name.clone(),
                            machine: from.to_string(),
                        },
                        format!(
                            "`{}` does not run on that machine; capture from one of its targets",
                            source.name
                        ),
                    ),
                    Err(e) => report.error(e, None),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{MachineId, NodeId};
    use crate::infra::Infra;
    use crate::node::NodeBuilder;
    use infrazeug_shell::{FileSource, ShellOp};
    use uuid::Uuid;

    fn infra_with(producer: NodeId, consumer_op: ShellOp, consumer: NodeId) -> Infra {
        let mut infra = Infra::new();
        infra = infra
            .add_node(
                NodeBuilder::shell(producer, ShellOp::run(vec!["gen".into()]), crate::node::Targets::All)
                    .build(),
            )
            .unwrap();
        infra = infra
            .add_node(
                NodeBuilder::shell(consumer, consumer_op, crate::node::Targets::All).build(),
            )
            .unwrap();
        infra
    }

    #[test]
    fn vault_write_capture_is_secret() {
        let producer = NodeId(Uuid::new_v4());
        let machine = MachineId(Uuid::new_v4());
        let consumer = NodeId(Uuid::new_v4());
        let store = ShellOp::VaultWrite {
            data_key_id: "k".into(),
            file: "f".into(),
            field: "secret_access_key".into(),
            value: FileSource::Capture(infrazeug_shell::CaptureRef {
                node: producer.0,
                machine: None,
            }),
            if_absent: false,
        };
        let infra = infra_with(producer, store, consumer);
        assert!(capture_is_secret(&infra, producer.0, machine.0));
    }

    #[test]
    fn plain_capture_is_not_secret() {
        let producer = NodeId(Uuid::new_v4());
        let machine = MachineId(Uuid::new_v4());
        let consumer = NodeId(Uuid::new_v4());
        let conf = ShellOp::WriteFile {
            path: "/etc/app/version".into(),
            content: FileSource::Capture(infrazeug_shell::CaptureRef {
                node: producer.0,
                machine: None,
            }),
            mode: 0o644,
        };
        let infra = infra_with(producer, conf, consumer);
        assert!(!capture_is_secret(&infra, producer.0, machine.0));
    }

    #[test]
    fn vault_write_with_explicit_other_machine_is_not_secret() {
        let producer = NodeId(Uuid::new_v4());
        let machine = MachineId(Uuid::new_v4());
        let other = Uuid::new_v4();
        let consumer = NodeId(Uuid::new_v4());
        let store = ShellOp::VaultWrite {
            data_key_id: "k".into(),
            file: "f".into(),
            field: "x".into(),
            value: FileSource::Capture(infrazeug_shell::CaptureRef {
                node: producer.0,
                machine: Some(other),
            }),
            if_absent: false,
        };
        let infra = infra_with(producer, store, consumer);
        // Same node id but a different machine than the ref names: not secret.
        assert!(!capture_is_secret(&infra, producer.0, machine.0));
        // The named machine is secret.
        assert!(capture_is_secret(&infra, producer.0, other));
    }
}
