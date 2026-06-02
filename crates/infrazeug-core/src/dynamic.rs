//! Dynamic machine groups: a discovery node plus a per-machine template that the
//! scheduler fans out at apply time once the discovered machines are known.
//!
//! The discovery node is an ordinary native node whose capture is a JSON array of
//! [`DiscoveredMachine`](crate::machine::DiscoveredMachine). The scheduler keys
//! expansion off [`DynamicGroup::discovery_node`]; there is no dedicated node body.

use crate::error::{CoreError, Result};
use crate::execution_graph::{ExecNode, ExecutionGraph, ExecutionGraphPatch, WorkUnit};
use crate::id::{MachineId, NodeId};
use crate::infra::{connect_node, connect_node_id, start_node_id};
use crate::machine::{Lifecycle, Machine, MachineKind};
use crate::node::{FailPolicy, Node, NodeBody, RunPolicy, Targets};
use uuid::Uuid;

/// Placeholder machine target carried by template nodes before per-machine
/// expansion. Template nodes never reach `resolve_targets` at plan time (they
/// live on [`DynamicGroup::template`], not in `Infra::nodes`), so the value only
/// needs to be a stable sentinel.
pub fn template_placeholder_machine() -> MachineId {
    MachineId(Uuid::nil())
}

/// Deterministic machine id for a discovered machine `name` within `label`.
pub fn dyn_machine_id(label: &str, name: &str) -> MachineId {
    let seed = format!("infrazeug/dyn/{label}/{name}");
    MachineId(Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes()))
}

/// Per-machine instance id for a template node bound to `machine`.
pub fn dyn_instance_node_id(template_node: NodeId, machine: MachineId) -> NodeId {
    let seed = format!("infrazeug/dyn-instance/{}/{}", template_node.0, machine.0);
    NodeId(Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes()))
}

/// Deterministic id of a dynamic group's exit barrier (downstream join point).
pub fn dyn_exit_node_id(label: &str) -> NodeId {
    let seed = format!("infrazeug/dyn-group/exit/{label}");
    NodeId(Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes()))
}

/// A dynamic machine group: a discovery node and a template fanned out per
/// discovered machine at apply time.
#[derive(Clone, Debug)]
pub struct DynamicGroup {
    /// Stable label; namespaces the group's machine and exit ids.
    pub label: String,
    /// Native node whose capture deserializes to `Vec<DiscoveredMachine>`.
    pub discovery_node: NodeId,
    /// Playbook body instantiated once per discovered machine. Nodes carry the
    /// placeholder target and template-relative deps; both are remapped when a
    /// machine is bound.
    pub template: Vec<Node>,
    /// External deps every per-machine head waits on. Always includes the
    /// discovery node so the fan-out runs after discovery.
    pub template_entry_deps: Vec<NodeId>,
    /// Per-machine failure handling (default tolerate-all: a bad machine is
    /// skipped, the rest proceed).
    pub fail_policy: FailPolicy,
    /// Cap on concurrently-running discovered machines (None = scheduler default).
    pub max_parallel_machines: Option<usize>,
}

impl DynamicGroup {
    /// Template nodes with no dependency on another template node — the per-machine
    /// heads that must additionally wait on [`template_entry_deps`](Self::template_entry_deps)
    /// and the machine's connect node.
    pub fn head_nodes(&self) -> impl Iterator<Item = &Node> {
        let ids: std::collections::HashSet<NodeId> = self.template.iter().map(|n| n.id).collect();
        self.template
            .iter()
            .filter(move |n| !n.deps.iter().any(|d| ids.contains(d)))
    }
}

/// The result of compiling a discovery node's capture into graph changes.
///
/// The scheduler registers [`machines_to_register`](Self::machines_to_register)
/// with the executor, applies [`graph_patch`](Self::graph_patch), and emits
/// `UnitsAdded` — it no longer owns any template-remapping mechanics
/// (recommendation 4).
#[derive(Default)]
pub struct DynamicExpansion {
    pub graph_patch: ExecutionGraphPatch,
    pub machines_to_register: Vec<Machine>,
}

/// Compile a dynamic group's discovery capture into an [`ExecutionGraphPatch`].
///
/// Reproduces the legacy in-scheduler fan-out as a pure transformation: it
/// deserializes the discovered machines, assigns deterministic machine/node ids,
/// synthesizes per-machine connect heads, remaps template deps, and extends the
/// group's exit barrier to join every per-machine leaf. The only inputs are the
/// `group`, the discovery `capture` bytes, and a read-only view of the current
/// `graph` (used to tell whether the global start node and exit barrier exist).
pub fn compile_expansion(
    group: &DynamicGroup,
    capture: &[u8],
    graph: &ExecutionGraph,
) -> Result<DynamicExpansion> {
    let machines: Vec<crate::machine::DiscoveredMachine> = serde_json::from_slice(capture)
        .map_err(|e| {
            CoreError::other(format!(
                "dynamic group `{}` discovery capture is not Vec<DiscoveredMachine>: {e}",
                group.label
            ))
        })?;

    let template_ids: std::collections::HashSet<NodeId> =
        group.template.iter().map(|n| n.id).collect();
    // Leaves: template nodes no other template node depends on.
    let template_leaves: Vec<NodeId> = group
        .template
        .iter()
        .filter(|n| !group.template.iter().any(|o| o.deps.contains(&n.id)))
        .map(|n| n.id)
        .collect();
    // A template-declared connect head, if the user provided one.
    let connect_tmpl: Option<NodeId> = group
        .template
        .iter()
        .find(|n| matches!(n.body, NodeBody::Connect))
        .map(|n| n.id);

    let start_id = start_node_id();
    let has_start = graph.nodes.contains_key(&start_id);
    let exit_id = dyn_exit_node_id(&group.label);

    let mut expansion = DynamicExpansion::default();
    let patch = &mut expansion.graph_patch;

    let push_node = |node: Node, machine_id: MachineId, patch: &mut ExecutionGraphPatch| {
        patch.units.push(WorkUnit::new(node.id, machine_id));
        patch.nodes.push(ExecNode::instantiated(node, machine_id));
    };

    for dm in &machines {
        let machine_id = dyn_machine_id(&group.label, &dm.name);
        let machine = Machine {
            id: machine_id,
            name: dm.name.clone(),
            kind: MachineKind::Remote {
                ssh: dm.ssh.clone(),
                os: dm.os.clone(),
            },
            vars: dm.vars.clone(),
            groups: Vec::new(),
            tags: dm.tags.clone(),
            max_parallel_nodes: None,
            lifecycle: Lifecycle::Persistent,
            like: None,
            lazy: true,
        };
        expansion.machines_to_register.push(machine);

        // Per-machine connect head: the template's connect node remapped, or a
        // synthesized one that waits on the discovery node.
        let connect_inst = match connect_tmpl {
            Some(tid) => dyn_instance_node_id(tid, machine_id),
            None => connect_node_id(machine_id),
        };
        if connect_tmpl.is_none() {
            let mut connect_deps = group.template_entry_deps.clone();
            if has_start && !connect_deps.contains(&start_id) {
                connect_deps.insert(0, start_id);
            }
            let mut cnode = connect_node(
                connect_inst,
                format!("connect/{}", dm.name),
                Targets::Machine(machine_id),
                connect_deps,
            );
            cnode.policy.fail_policy = group.fail_policy;
            push_node(cnode, machine_id, patch);
        }

        for t in &group.template {
            let new_id = dyn_instance_node_id(t.id, machine_id);
            let mut node = t.clone();
            node.id = new_id;
            node.targets = Targets::Machine(machine_id);
            node.policy.fail_policy = group.fail_policy;
            let intra: Vec<NodeId> = t
                .deps
                .iter()
                .filter(|d| template_ids.contains(d))
                .map(|d| dyn_instance_node_id(*d, machine_id))
                .collect();
            let is_connect = matches!(t.body, NodeBody::Connect);
            let is_head = !t.deps.iter().any(|d| template_ids.contains(d));
            let is_lazy = matches!(t.policy.run_policy, RunPolicy::Lazy);
            node.deps = if is_connect {
                // Connect head waits on the discovery node (+ entry deps).
                let mut d = intra;
                if has_start && !d.contains(&start_id) {
                    d.push(start_id);
                }
                d.extend(group.template_entry_deps.iter().copied());
                d
            } else if is_head && !is_lazy {
                // Non-connect head waits on this machine's connect node.
                vec![connect_inst]
            } else {
                intra
            };
            push_node(node, machine_id, patch);
        }

        // Extend the exit barrier so downstream waits for every per-machine leaf.
        for leaf in &template_leaves {
            patch
                .edges
                .push((dyn_instance_node_id(*leaf, machine_id), exit_id));
        }
    }

    Ok(expansion)
}
