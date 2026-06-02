//! Internal execution IR lowered from a validated [`ExecutablePlan`].
//!
//! [`ExecutionGraph`] is the normalized, scheduler-ready representation that sits
//! between [`Plan::executable`](crate::plan::Plan::executable) and
//! [`DefaultScheduler::run`](crate::scheduler::DefaultScheduler). It owns one
//! already-resolved view of the graph — targets resolved into per-machine
//! [`WorkUnit`]s, public [`NodeBody`] lowered into a smaller [`NodeAction`] model,
//! and dependency adjacency precomputed — so the scheduler no longer rebuilds
//! `node_by_id`, `planned_by_id`, `run_policy_by_id`, `dependents_by_id`, and
//! `work` inline.
//!
//! See `docs/node-architecture-simplification.md` (recommendations 1, 3, 4). The
//! migration compiles this IR down to the scheduler's current microarchitecture
//! via [`ExecutionGraph::to_scheduler_compat`]; dynamic fan-out lands as an
//! [`ExecutionGraphPatch`].

use crate::id::{MachineId, NodeId};
use crate::infra::{end_node_id, start_node_id};
use crate::machine::Machine;
use crate::node::{Node, NodeBody, NodePolicy, NodeSummary, PlanOutcome, RunPolicy};
use crate::plan::{ExecutablePlan, NodeFingerprint, PlannedNode};
use infrazeug_shell::ShellOp;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

/// Stable identity of one unit of work: a node on a specific target machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorkKey {
    pub node_id: NodeId,
    pub machine_id: MachineId,
}

impl WorkKey {
    pub fn new(node_id: NodeId, machine_id: MachineId) -> Self {
        Self {
            node_id,
            machine_id,
        }
    }
}

/// Lowered, scheduler-internal action model (recommendation 3).
///
/// Public [`NodeBody`] variants stay for compatibility, but the scheduler reasons
/// over this smaller model: every node is either remote *work* ([`Exec`]), a
/// *system* probe ([`System`]), or a graph-only *no-op* ([`Noop`]). Lowering the
/// three graph-only bodies into a single [`Noop`] path keeps their display roles
/// while collapsing scheduling and execution to one branch.
///
/// [`Exec`]: NodeAction::Exec
/// [`System`]: NodeAction::System
/// [`Noop`]: NodeAction::Noop
#[derive(Clone, Debug)]
pub enum NodeAction {
    Exec(ExecAction),
    System(SystemAction),
    Noop(NoopRole),
}

/// Remote user work: a serializable [`Shell`](ExecAction::Shell) op or a typed
/// [`Native`](ExecAction::Native) method. Their transport and serialization rules
/// stay intentionally distinct (see the doc's "Avoid These Simplifications").
#[derive(Clone, Debug)]
pub enum ExecAction {
    Shell(ShellOp),
    Native {
        method_id: String,
        input: serde_cbor::Value,
    },
}

/// System-level actions that are real graph nodes but not user work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemAction {
    /// Connectivity / agent-upload probe — a machine's first transport use.
    Connect,
}

/// Display role of a graph-only [`Noop`](NodeAction::Noop) node.
///
/// `Start`/`End` are the global execution-graph bookends (a [`NodeBody::Begin`] at
/// [`start_node_id`] and a [`NodeBody::Finish`] at [`end_node_id`]); the remaining
/// roles map straight from their body so DOT/TUI labels stay meaningful.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoopRole {
    Barrier,
    Begin,
    Finish,
    Start,
    End,
}

impl NodeAction {
    /// Lower a public [`Node`] into its internal action.
    ///
    /// Takes the whole node (not just the body) so the global start/end bookends
    /// can be tagged [`NoopRole::Start`]/[`NoopRole::End`] by id.
    pub fn from_node(node: &Node) -> Self {
        match &node.body {
            NodeBody::Shell(op) => NodeAction::Exec(ExecAction::Shell(op.clone())),
            NodeBody::Native { method_id, input } => NodeAction::Exec(ExecAction::Native {
                method_id: method_id.clone(),
                input: input.clone(),
            }),
            NodeBody::Connect => NodeAction::System(SystemAction::Connect),
            NodeBody::Barrier => NodeAction::Noop(noop_role(node.id, NoopRole::Barrier)),
            NodeBody::Begin => NodeAction::Noop(noop_role(node.id, NoopRole::Begin)),
            NodeBody::Finish => NodeAction::Noop(noop_role(node.id, NoopRole::Finish)),
        }
    }

    /// Graph-only node performing no remote work (barrier or group bookend).
    pub fn is_graph_only(&self) -> bool {
        matches!(self, NodeAction::Noop(_))
    }

    /// Connectivity / agent-upload system probe.
    pub fn is_connect(&self) -> bool {
        matches!(self, NodeAction::System(SystemAction::Connect))
    }

    /// User-authored remote work (shell or native).
    pub fn is_user_work(&self) -> bool {
        matches!(self, NodeAction::Exec(_))
    }
}

fn noop_role(id: NodeId, default: NoopRole) -> NoopRole {
    if id == start_node_id() {
        NoopRole::Start
    } else if id == end_node_id() {
        NoopRole::End
    } else {
        default
    }
}

/// One node in the [`ExecutionGraph`]: its lowered action, scheduling policy,
/// resolved dependencies, and canonical plan entry.
///
/// During the migration the original [`Node`] is retained beside [`action`] so the
/// scheduler can keep handing the existing executor a `Node` unchanged.
///
/// [`action`]: ExecNode::action
#[derive(Clone, Debug)]
pub struct ExecNode {
    pub id: NodeId,
    pub summary: NodeSummary,
    pub action: NodeAction,
    pub policy: NodePolicy,
    pub deps: Vec<NodeId>,
    pub planned: PlannedNode,
    /// Original authoring node, retained for the transition (recommendation 1).
    ///
    /// `Arc`-shared so that cloning the graph, materializing the compat lookup
    /// maps ([`ExecutionGraph::to_scheduler_compat`]), and the scheduler's
    /// per-unit dispatch clone refcount-bump instead of deep-cloning the whole
    /// node each time. The first materialization in [`ExecNode::new`] /
    /// [`ExecutionGraph::from_executable`] still pays one deep clone from the
    /// borrowed plan node.
    pub node: Arc<Node>,
}

impl ExecNode {
    /// Build an execution node from a [`Node`] and its canonical plan entry.
    pub fn new(node: Node, planned: PlannedNode) -> Self {
        ExecNode {
            id: node.id,
            summary: NodeSummary::from_node(&node),
            action: NodeAction::from_node(&node),
            policy: node.policy.clone(),
            deps: node.deps.clone(),
            planned,
            node: Arc::new(node),
        }
    }

    /// Build an execution node for a dynamically instantiated per-machine node.
    ///
    /// Synthesizes a single-machine, `Unknown`-outcome plan entry (fan-out nodes
    /// have no plan-time fingerprint), matching the legacy inline expansion.
    pub fn instantiated(node: Node, machine_id: MachineId) -> Self {
        let planned = PlannedNode {
            node_id: node.id,
            name: node.name.clone(),
            description: node.description.clone(),
            machines: vec![machine_id],
            outcome: PlanOutcome::Unknown,
            fingerprint: NodeFingerprint::default(),
        };
        ExecNode::new(node, planned)
    }

    pub fn run_policy(&self) -> RunPolicy {
        self.policy.run_policy
    }

    /// Demand-driven node ([`RunPolicy::Lazy`]) — dormant until pulled.
    pub fn is_lazy(&self) -> bool {
        matches!(self.policy.run_policy, RunPolicy::Lazy)
    }

    pub fn is_graph_only(&self) -> bool {
        self.action.is_graph_only()
    }

    pub fn is_connect(&self) -> bool {
        self.action.is_connect()
    }
}

/// One unit of work: a node resolved onto a specific machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkUnit {
    pub key: WorkKey,
    pub node_id: NodeId,
    pub machine_id: MachineId,
}

impl WorkUnit {
    pub fn new(node_id: NodeId, machine_id: MachineId) -> Self {
        Self {
            key: WorkKey::new(node_id, machine_id),
            node_id,
            machine_id,
        }
    }
}

/// Normalized, target-resolved, scheduler-ready graph (recommendation 1).
#[derive(Clone, Debug, Default)]
pub struct ExecutionGraph {
    pub nodes: FxHashMap<NodeId, ExecNode>,
    pub units: FxHashMap<WorkKey, WorkUnit>,
    pub dependents: FxHashMap<NodeId, Vec<NodeId>>,
}

impl ExecutionGraph {
    /// Lower a validated [`ExecutablePlan`] into the execution graph.
    ///
    /// One [`ExecNode`] per planned node, one [`WorkUnit`] per `(node, machine)`,
    /// and `dep -> dependent` adjacency built from each node's deps.
    pub fn from_executable(exec: &ExecutablePlan<'_>) -> Self {
        let mut nodes =
            FxHashMap::with_capacity_and_hasher(exec.planned_by_id.len(), Default::default());
        // Pre-size `units` to the exact (node, machine) count so the fan-out loop
        // never reallocates the map (it previously started empty and grew).
        let unit_capacity: usize = exec.planned_by_id.values().map(|p| p.machines.len()).sum();
        let mut units = FxHashMap::with_capacity_and_hasher(unit_capacity, Default::default());

        for (&id, planned) in &exec.planned_by_id {
            // `executable()` guarantees every planned node exists in infra.
            let Some(node) = exec.node_by_id.get(&id).copied() else {
                continue;
            };
            for &machine_id in &planned.machines {
                let unit = WorkUnit::new(id, machine_id);
                units.insert(unit.key, unit);
            }
            nodes.insert(
                id,
                ExecNode {
                    id,
                    summary: NodeSummary::from_node(node),
                    action: NodeAction::from_node(node),
                    policy: node.policy.clone(),
                    deps: node.deps.clone(),
                    planned: (*planned).clone(),
                    node: Arc::new(node.clone()),
                },
            );
        }

        let dependents = build_dependents(nodes.values().map(|n| (n.id, n.deps.as_slice())));

        ExecutionGraph {
            nodes,
            units,
            dependents,
        }
    }

    /// Materialize the scheduler's legacy lookup maps (migration backend).
    ///
    /// `DefaultScheduler::run` consumes these instead of rebuilding them inline.
    pub fn to_scheduler_compat(&self) -> SchedulerCompat {
        let mut node_by_id =
            FxHashMap::with_capacity_and_hasher(self.nodes.len(), Default::default());
        let mut planned_by_id =
            FxHashMap::with_capacity_and_hasher(self.nodes.len(), Default::default());
        let mut run_policy_by_id =
            FxHashMap::with_capacity_and_hasher(self.nodes.len(), Default::default());
        for (&id, n) in &self.nodes {
            node_by_id.insert(id, n.node.clone());
            planned_by_id.insert(id, n.planned.clone());
            run_policy_by_id.insert(id, n.policy.run_policy);
        }
        SchedulerCompat {
            node_by_id,
            planned_by_id,
            run_policy_by_id,
            dependents_by_id: self.dependents.clone(),
            work: self.units.keys().copied().collect(),
        }
    }

    /// Apply a dynamic fan-out patch (recommendation 4).
    ///
    /// New nodes register their own deps into the adjacency; `edges` extend an
    /// existing node's deps (used to grow a dynamic group's exit barrier to join
    /// every per-machine leaf). Discovered [`machines`](ExecutionGraphPatch::machines)
    /// are registered with the executor by the scheduler, not stored here.
    pub fn apply_patch(&mut self, patch: ExecutionGraphPatch) {
        for node in patch.nodes {
            for &dep in &node.deps {
                self.dependents.entry(dep).or_default().push(node.id);
            }
            self.nodes.insert(node.id, node);
        }
        for unit in patch.units {
            self.units.insert(unit.key, unit);
        }
        for (from, to) in patch.edges {
            if let Some(node) = self.nodes.get_mut(&to) {
                if !node.deps.contains(&from) {
                    node.deps.push(from);
                    self.dependents.entry(from).or_default().push(to);
                }
            }
        }
    }
}

/// Build `dep -> dependents` adjacency from `(node_id, deps)` pairs.
fn build_dependents<'a>(
    nodes: impl Iterator<Item = (NodeId, &'a [NodeId])>,
) -> FxHashMap<NodeId, Vec<NodeId>> {
    let mut dependents: FxHashMap<NodeId, Vec<NodeId>> = FxHashMap::default();
    for (id, deps) in nodes {
        for &dep in deps {
            dependents.entry(dep).or_default().push(id);
        }
    }
    dependents
}

/// Legacy scheduler lookup maps, materialized from an [`ExecutionGraph`].
///
/// This is the transitional backend: the scheduler dispatch loop still reads these
/// while the IR is introduced one piece at a time. Removed once the loop consumes
/// [`ExecutionGraph`] natively.
pub struct SchedulerCompat {
    pub node_by_id: FxHashMap<NodeId, Arc<Node>>,
    pub planned_by_id: FxHashMap<NodeId, PlannedNode>,
    pub run_policy_by_id: FxHashMap<NodeId, RunPolicy>,
    pub dependents_by_id: FxHashMap<NodeId, Vec<NodeId>>,
    pub work: FxHashSet<WorkKey>,
}

/// A graph mutation produced by dynamic fan-out (recommendation 4).
#[derive(Default)]
pub struct ExecutionGraphPatch {
    /// Freshly instantiated per-machine nodes.
    pub nodes: Vec<ExecNode>,
    /// New units for the inserted nodes.
    pub units: Vec<WorkUnit>,
    /// `(dep, dependent)` edges that extend an *existing* node's dependencies.
    pub edges: Vec<(NodeId, NodeId)>,
    /// Machines discovered at apply time, to register with the executor.
    pub machines: Vec<Machine>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{MachineId, NodeId};
    use crate::infra::{end_node, start_node, Infra};
    use crate::node::{NodeBuilder, PlanOutcome, Targets};
    use crate::plan::{NodeFingerprint, Plan, PlanDigest, PlannedNode};
    use infrazeug_shell::ShellOp;
    use uuid::Uuid;

    fn planned_on(node_id: NodeId, machines: Vec<MachineId>) -> PlannedNode {
        PlannedNode {
            node_id,
            name: node_id.to_string(),
            description: None,
            machines,
            outcome: PlanOutcome::Unknown,
            fingerprint: NodeFingerprint::default(),
        }
    }

    fn exec_graph(infra: &Infra, plan: &Plan) -> ExecutionGraph {
        let exec = plan.executable(infra).expect("executable");
        ExecutionGraph::from_executable(&exec)
    }

    #[test]
    fn fans_out_one_unit_per_machine_and_builds_adjacency() {
        let m1 = MachineId(Uuid::new_v4());
        let m2 = MachineId(Uuid::new_v4());
        let base = NodeId(Uuid::new_v4());
        let dependent = NodeId(Uuid::new_v4());

        let mut infra = Infra::new();
        infra.nodes.push(
            NodeBuilder::shell(base, ShellOp::run(vec!["true".into()]), Targets::All).build(),
        );
        infra.nodes.push(
            NodeBuilder::shell(dependent, ShellOp::run(vec!["true".into()]), Targets::All)
                .deps(vec![base])
                .build(),
        );
        let plan = Plan {
            digest: PlanDigest([0; 32]),
            nodes: vec![
                planned_on(base, vec![m1, m2]),
                planned_on(dependent, vec![m1]),
            ],
            signatures: vec![],
        };

        let graph = exec_graph(&infra, &plan);

        // One unit per (node, machine): base on m1+m2, dependent on m1.
        assert_eq!(graph.units.len(), 3);
        assert!(graph.units.contains_key(&WorkKey::new(base, m1)));
        assert!(graph.units.contains_key(&WorkKey::new(base, m2)));
        assert!(graph.units.contains_key(&WorkKey::new(dependent, m1)));

        // dep -> dependent adjacency.
        assert_eq!(graph.dependents.get(&base), Some(&vec![dependent]));

        let compat = graph.to_scheduler_compat();
        assert_eq!(compat.work.len(), 3);
        assert_eq!(compat.node_by_id.len(), 2);
        assert_eq!(compat.dependents_by_id.get(&base), Some(&vec![dependent]));
    }

    #[test]
    fn lowers_bodies_to_actions_with_display_roles() {
        let m = MachineId(Uuid::new_v4());
        let shell = NodeId(Uuid::new_v4());
        let native = NodeId(Uuid::new_v4());
        let connect = NodeId(Uuid::new_v4());
        let barrier = NodeId(Uuid::new_v4());

        let mut infra = Infra::new();
        infra.nodes.push(
            NodeBuilder::shell(shell, ShellOp::run(vec!["true".into()]), Targets::All).build(),
        );
        infra
            .nodes
            .push(NodeBuilder::native(native, "demo.method", Targets::All).build());
        infra
            .nodes
            .push(NodeBuilder::connect(connect, Targets::All).build());
        infra
            .nodes
            .push(NodeBuilder::barrier(barrier, Targets::All).build());
        // Global bookends get Start/End roles by id.
        infra.nodes.push(start_node());
        infra.nodes.push(end_node(vec![barrier]));

        let plan = Plan {
            digest: PlanDigest([0; 32]),
            nodes: vec![
                planned_on(shell, vec![m]),
                planned_on(native, vec![m]),
                planned_on(connect, vec![m]),
                planned_on(barrier, vec![m]),
                planned_on(crate::infra::start_node_id(), vec![m]),
                planned_on(crate::infra::end_node_id(), vec![m]),
            ],
            signatures: vec![],
        };
        let graph = exec_graph(&infra, &plan);

        assert!(graph.nodes[&shell].action.is_user_work());
        assert!(graph.nodes[&native].action.is_user_work());
        assert!(graph.nodes[&connect].action.is_connect());
        assert!(matches!(
            graph.nodes[&barrier].action,
            NodeAction::Noop(NoopRole::Barrier)
        ));
        assert!(matches!(
            graph.nodes[&crate::infra::start_node_id()].action,
            NodeAction::Noop(NoopRole::Start)
        ));
        assert!(matches!(
            graph.nodes[&crate::infra::end_node_id()].action,
            NodeAction::Noop(NoopRole::End)
        ));
    }

    #[test]
    fn apply_patch_adds_nodes_units_and_extends_exit_barrier() {
        let m = MachineId(Uuid::new_v4());
        let exit = NodeId(Uuid::new_v4());

        let mut infra = Infra::new();
        infra
            .nodes
            .push(NodeBuilder::barrier(exit, Targets::All).build());
        let plan = Plan {
            digest: PlanDigest([0; 32]),
            nodes: vec![planned_on(exit, vec![m])],
            signatures: vec![],
        };
        let mut graph = exec_graph(&infra, &plan);

        let dm = MachineId(Uuid::new_v4());
        let leaf = NodeId(Uuid::new_v4());
        let leaf_node =
            NodeBuilder::shell(leaf, ShellOp::run(vec!["true".into()]), Targets::All).build();
        let exec = ExecNode {
            id: leaf,
            summary: NodeSummary::from_node(&leaf_node),
            action: NodeAction::from_node(&leaf_node),
            policy: leaf_node.policy.clone(),
            deps: leaf_node.deps.clone(),
            planned: planned_on(leaf, vec![dm]),
            node: Arc::new(leaf_node),
        };
        let patch = ExecutionGraphPatch {
            nodes: vec![exec],
            units: vec![WorkUnit::new(leaf, dm)],
            edges: vec![(leaf, exit)],
            machines: vec![],
        };
        graph.apply_patch(patch);

        assert!(graph.units.contains_key(&WorkKey::new(leaf, dm)));
        assert!(graph.nodes[&exit].deps.contains(&leaf));
        assert_eq!(graph.dependents.get(&leaf), Some(&vec![exit]));
    }
}
