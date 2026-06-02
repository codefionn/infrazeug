//! Node stdout captures for cross-node / cross-machine `WriteFile` (SOUL §3.3.3).

use crate::error::{CoreError, Result};
use crate::id::{MachineId, NodeId};
use infrazeug_shell::{capture_refs, CAPTURE_MAX_BYTES};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

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
                tokio::fs::write(&path, &stdout).await?;
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
