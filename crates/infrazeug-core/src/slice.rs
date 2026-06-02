//! Per-machine plan slices and `WaitForHash` (SOUL §3.10.2).
//!
//! # Slicing microarchitecture
//!
//! After the controller builds a [`Plan`] (topological sort of all nodes),
//! it **slices** the plan per target machine via
//! [`Plan::slice_for_machine`]. Each [`PlanSlice`] carries only the steps
//! relevant to that machine plus, in push mode, `WaitForHash` markers for
//! cross-machine dependencies resolved at apply time by the [`HashRelay`].
//!
//! Pull mode forbids cross-machine deps — `slice_for_machine` returns
//! `PullSliceNeedsWait` if one is encountered — so pull slices never
//! contain `WaitForHash` steps.
//!
//! ```text
//!   Infra.graph → Plan → PlanSlice (per machine) → apply
//! ```
//!
//! Slices are serialized as CBOR for pull-mode sealed blobs and
//! reconstituted via [`slice_to_plan`] on the apply side. See
//! `docs/protocol.md` for the full microarchitecture.

use crate::error::{CoreError, Result};
use crate::id::{MachineId, NodeId};
use crate::infra::Infra;
use crate::machine::Machine;
use crate::node::Node;
use crate::plan::{Plan, PlanDigest, PlannedNode};
use crate::varset::VarValue;
use infrazeug_secrets::PlanSignature;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SliceMode {
    /// Cross-machine deps become `WaitForHash` markers.
    Push,
    /// Fails if a wait would be required.
    Pull,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WaitId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sha256Digest(pub [u8; 32]);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SliceStep {
    Node(PlannedNode),
    WaitForHash {
        id: WaitId,
        expect: Sha256Digest,
        sources: Vec<MachineId>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanSlice {
    pub machine_id: MachineId,
    pub digest: PlanDigest,
    pub steps: Vec<SliceStep>,
    /// Content-addressed custom agent (pull-mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_digest: Option<String>,
    /// Inlined secret vars for pull-mode (`key` → CBOR value).
    #[serde(default)]
    pub inlined_vars: HashMap<String, serde_cbor::Value>,
    #[serde(default)]
    pub signatures: Vec<PlanSignature>,
    /// Node bodies for pull apply (host has no controller `Infra`).
    #[serde(default)]
    pub embedded_nodes: Vec<Node>,
    #[serde(default)]
    pub embedded_machine: Option<Machine>,
}

impl PlanSlice {
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        Ok(serde_cbor::to_vec(self)?)
    }

    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        Ok(serde_cbor::from_slice(bytes)?)
    }

    pub fn finalize(mut self) -> Self {
        self.digest = slice_digest(&self);
        self
    }
}

pub fn slice_digest(slice: &PlanSlice) -> PlanDigest {
    let mut steps = slice.steps.clone();
    for s in &mut steps {
        if let SliceStep::Node(n) = s {
            n.outcome = crate::node::PlanOutcome::Unknown;
        }
    }
    let mut copy = slice.clone();
    copy.steps = steps;
    copy.inlined_vars.clear();
    // The digest is what gets signed, so it must not depend on the signatures
    // themselves (appended after signing) nor on the digest field's prior value
    // — otherwise it would not be reproducible after a re-finalize or roundtrip.
    copy.signatures.clear();
    copy.digest = PlanDigest([0; 32]);
    let bytes = serde_cbor::to_vec(&copy).expect("slice cbor");
    let hash = Sha256::digest(&bytes);
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&hash);
    PlanDigest(digest)
}

impl Plan {
    pub fn slice_for_machine(
        &self,
        infra: &Infra,
        machine_id: MachineId,
        mode: SliceMode,
    ) -> Result<PlanSlice> {
        let node_by_id: FxHashMap<NodeId, &Node> = infra.nodes.iter().map(|n| (n.id, n)).collect();
        let planned_by_id: FxHashMap<NodeId, &PlannedNode> =
            self.nodes.iter().map(|p| (p.node_id, p)).collect();

        let on_machine: FxHashSet<NodeId> = self
            .nodes
            .iter()
            .filter(|p| p.machines.contains(&machine_id))
            .map(|p| p.node_id)
            .collect();

        let mut steps = Vec::new();
        let mut order: Vec<NodeId> = Vec::new();
        for p in &self.nodes {
            if on_machine.contains(&p.node_id) {
                order.push(p.node_id);
            }
        }

        for node_id in order {
            let node = node_by_id
                .get(&node_id)
                .ok_or_else(|| CoreError::other("slice: node missing from infra"))?;
            let planned = planned_by_id
                .get(&node_id)
                .ok_or_else(|| CoreError::other("slice: node missing from plan"))?;
            if mode == SliceMode::Pull && node_contains_controller_sync(node) {
                return Err(CoreError::other(format!(
                    "pull slice for `{}` contains controller-local SyncDir (use push mode)",
                    node.name
                )));
            }

            for dep in &node.deps {
                if on_machine.contains(dep) {
                    continue;
                }
                let dep_node = node_by_id
                    .get(dep)
                    .ok_or_else(|| CoreError::other("slice: dep missing"))?;
                let dep_planned = planned_by_id
                    .get(dep)
                    .ok_or_else(|| CoreError::other("slice: dep missing from plan"))?;

                match mode {
                    SliceMode::Pull => {
                        return Err(CoreError::PullSliceNeedsWait {
                            node: node.name.clone(),
                            dependency: dep_node.name.clone(),
                        });
                    }
                    SliceMode::Push => {
                        let sources: Vec<MachineId> = dep_planned.machines.clone();
                        let expect = completion_digest(dep, &sources);
                        let id = wait_id(*dep, machine_id);
                        steps.push(SliceStep::WaitForHash {
                            id,
                            expect: Sha256Digest(expect),
                            sources,
                        });
                    }
                }
            }

            steps.push(SliceStep::Node((*planned).clone()));
        }

        let mut inlined_vars = HashMap::new();
        let mut embedded_nodes = Vec::new();
        let mut embedded_machine = None;
        if mode == SliceMode::Pull {
            let machine = infra
                .machine_by_id(machine_id)
                .ok_or_else(|| CoreError::other("unknown machine for pull slice"))?
                .clone();
            embedded_machine = Some(machine.clone());
            for (k, entry) in &machine.vars.entries {
                if let VarValue::Vault(r) = &entry.value {
                    inlined_vars.insert(
                        k.0.clone(),
                        serde_cbor::Value::Text(format!(
                            "vault:{}#{}",
                            r.file,
                            r.field.as_deref().unwrap_or("value")
                        )),
                    );
                }
            }
            for node_id in &on_machine {
                if let Some(n) = node_by_id.get(node_id) {
                    embedded_nodes.push((*n).clone());
                }
            }
        }

        Ok(PlanSlice {
            machine_id,
            digest: PlanDigest([0; 32]),
            steps,
            agent_digest: None,
            inlined_vars,
            signatures: Vec::new(),
            embedded_nodes,
            embedded_machine,
        }
        .finalize())
    }
}

fn node_contains_controller_sync(node: &Node) -> bool {
    match &node.body {
        crate::node::NodeBody::Shell(op) => shell_op_contains_controller_sync(op),
        _ => false,
    }
}

fn shell_op_contains_controller_sync(op: &infrazeug_shell::ShellOp) -> bool {
    match op {
        infrazeug_shell::ShellOp::SyncDir { .. } => true,
        infrazeug_shell::ShellOp::Seq { steps } => {
            steps.iter().any(shell_op_contains_controller_sync)
        }
        _ => false,
    }
}

fn wait_id(dep: NodeId, target: MachineId) -> WaitId {
    let mut h = Sha256::new();
    h.update(dep.0.as_bytes());
    h.update(target.0.as_bytes());
    let bytes = h.finalize();
    WaitId(u64::from_be_bytes(bytes[0..8].try_into().unwrap()))
}

pub fn completion_digest(dep: &NodeId, sources: &[MachineId]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"node-completion-v1");
    h.update(dep.0.as_bytes());
    for m in sources {
        h.update(m.0.as_bytes());
    }
    h.finalize().into()
}

pub fn slice_to_plan(slice: &PlanSlice) -> Plan {
    let nodes: Vec<PlannedNode> = slice
        .steps
        .iter()
        .filter_map(|s| match s {
            SliceStep::Node(n) => Some(n.clone()),
            SliceStep::WaitForHash { .. } => None,
        })
        .collect();
    Plan {
        digest: slice.digest,
        nodes,
        signatures: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::NodeId;
    use crate::node::{Node, NodeBody, Targets};
    use infrazeug_shell::ShellOp;
    use uuid::Uuid;

    fn mid() -> MachineId {
        MachineId(Uuid::new_v4())
    }

    fn nid() -> NodeId {
        NodeId(Uuid::new_v4())
    }

    #[test]
    fn pull_rejects_cross_machine_dep() {
        let a = mid();
        let b = mid();
        let n1 = nid();
        let n2 = nid();
        let mut infra = Infra::new();
        infra.machines.push(crate::machine::Machine {
            id: a,
            name: "a".into(),
            kind: crate::machine::MachineKind::Local,
            vars: Default::default(),
            groups: vec![],
            tags: vec![],
            max_parallel_nodes: None,
            lifecycle: crate::machine::Lifecycle::Persistent,
            like: None,
            lazy: false,
        });
        infra.machines.push(crate::machine::Machine {
            id: b,
            name: "b".into(),
            kind: crate::machine::MachineKind::Local,
            vars: Default::default(),
            groups: vec![],
            tags: vec![],
            max_parallel_nodes: None,
            lifecycle: crate::machine::Lifecycle::Persistent,
            like: None,
            lazy: false,
        });
        infra.nodes.push(Node {
            id: n1,
            name: "on-a".into(),
            description: None,
            body: NodeBody::Shell(ShellOp::Run {
                argv: vec!["true".into()],
                cwd: None,
                env: Vec::new(),
            }),
            targets: Targets::Machine(a),
            deps: vec![],
            tags: vec![],
            policy: Default::default(),
        });
        infra.nodes.push(Node {
            id: n2,
            name: "on-b-needs-a".into(),
            description: None,
            body: NodeBody::Shell(ShellOp::Run {
                argv: vec!["true".into()],
                cwd: None,
                env: Vec::new(),
            }),
            targets: Targets::Machine(b),
            deps: vec![n1],
            tags: vec![],
            policy: Default::default(),
        });
        let plan = infra.plan().unwrap();
        assert!(plan.slice_for_machine(&infra, b, SliceMode::Pull).is_err());
        let slice = plan.slice_for_machine(&infra, b, SliceMode::Push).unwrap();
        assert!(slice
            .steps
            .iter()
            .any(|s| matches!(s, SliceStep::WaitForHash { .. })));
    }

    #[test]
    fn slice_digest_ignores_inlined_vars() {
        let mid = mid();
        let mut a = PlanSlice {
            machine_id: mid,
            digest: PlanDigest([0; 32]),
            steps: vec![],
            agent_digest: None,
            inlined_vars: HashMap::from([("k".into(), serde_cbor::Value::Text("v".into()))]),
            signatures: vec![],
            embedded_nodes: vec![],
            embedded_machine: None,
        };
        let d1 = slice_digest(&a);
        a.inlined_vars.clear();
        let d2 = slice_digest(&a);
        assert_eq!(d1, d2);
    }

    #[test]
    fn slice_digest_ignores_signatures() {
        let a = PlanSlice {
            machine_id: mid(),
            digest: PlanDigest([0; 32]),
            steps: vec![],
            agent_digest: None,
            inlined_vars: HashMap::new(),
            signatures: vec![],
            embedded_nodes: vec![],
            embedded_machine: None,
        };
        let mut b = a.clone();
        b.signatures.push(PlanSignature {
            signer_id: "x".into(),
            public_key: [1u8; 32],
            signature: vec![2u8; 64],
        });
        // Adding a signature must not change the signed digest.
        assert_eq!(slice_digest(&a), slice_digest(&b));
    }

    #[test]
    fn slice_to_plan_strips_waits() {
        let n = PlannedNode {
            node_id: nid(),
            name: "n".into(),
            description: None,
            machines: vec![mid()],
            outcome: crate::node::PlanOutcome::Unknown,
            fingerprint: Default::default(),
        };
        let slice = PlanSlice {
            machine_id: mid(),
            digest: PlanDigest([0; 32]),
            steps: vec![
                SliceStep::Node(n.clone()),
                SliceStep::WaitForHash {
                    id: WaitId(1),
                    expect: Sha256Digest([1u8; 32]),
                    sources: vec![mid()],
                },
            ],
            agent_digest: None,
            inlined_vars: HashMap::new(),
            signatures: vec![],
            embedded_nodes: vec![],
            embedded_machine: None,
        };
        let plan = slice_to_plan(&slice);
        assert_eq!(plan.nodes.len(), 1);
        assert_eq!(plan.nodes[0].name, "n");
    }

    #[test]
    fn pull_allows_same_machine_dep() {
        let m = mid();
        let n1 = nid();
        let n2 = nid();
        let mut infra = Infra::new();
        infra.machines.push(crate::machine::Machine {
            id: m,
            name: "solo".into(),
            kind: crate::machine::MachineKind::Local,
            vars: Default::default(),
            groups: vec![],
            tags: vec![],
            max_parallel_nodes: None,
            lifecycle: crate::machine::Lifecycle::Persistent,
            like: None,
            lazy: false,
        });
        infra.nodes.push(Node {
            id: n1,
            name: "first".into(),
            description: None,
            body: NodeBody::Shell(ShellOp::Run {
                argv: vec!["true".into()],
                cwd: None,
                env: Vec::new(),
            }),
            targets: Targets::Machine(m),
            deps: vec![],
            tags: vec![],
            policy: Default::default(),
        });
        infra.nodes.push(Node {
            id: n2,
            name: "second".into(),
            description: None,
            body: NodeBody::Shell(ShellOp::Run {
                argv: vec!["true".into()],
                cwd: None,
                env: Vec::new(),
            }),
            targets: Targets::Machine(m),
            deps: vec![n1],
            tags: vec![],
            policy: Default::default(),
        });
        let plan = infra.plan().unwrap();
        let slice = plan.slice_for_machine(&infra, m, SliceMode::Pull).unwrap();
        assert!(slice.steps.iter().all(|s| matches!(s, SliceStep::Node(_))));
    }
}
