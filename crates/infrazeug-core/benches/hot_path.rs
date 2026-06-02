//! Hot-path microbenchmarks for the planning/scheduling engine.
//!
//! These cover the pure-CPU work that runs on every `plan`/`apply`: node
//! fingerprinting, plan digest + (de)serialization, lowering an
//! [`ExecutablePlan`] into the scheduler-ready [`ExecutionGraph`], the
//! per-tick pure decision engine ([`GraphState`]), the whole-infra lint pass,
//! graph-view build/filter/render, and per-machine plan slicing. All of it is
//! allocation-heavy (it clones the graph and rebuilds lookup maps), so we run
//! it under divan's [`AllocProfiler`] to report **both time and allocations**
//! per size.
//!
//! Run with:
//!   cargo bench -p infrazeug-core --bench hot_path
//!   cargo bench -p infrazeug-core --bench hot_path -- decision   # filter
//!
//! Each row prints median/mean wall time plus alloc count and bytes
//! (allocations / grows / shrinks) for the benchmarked closure only — fixture
//! construction happens outside the timed region.

use rustc_hash::FxHashMap;

use divan::Bencher;
use infrazeug_core::id::RawUuid;
use infrazeug_core::machine::Lifecycle;
use infrazeug_core::{
    node_fingerprint, plan_digest, slice_digest, ExecutionGraph, GraphSelect, GraphState, Infra,
    Machine, MachineId, MachineKind, Node, NodeBuilder, NodeId, NodeStatus, Plan, PlanDigest,
    PlanOutcome, PlanSlice, PlannedNode, RunPolicy, SliceMode, Targets, VarSet, WorkKey,
};
use infrazeug_shell::ShellOp;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

/// Total node counts to sweep. Each layer is [`WIDTH`] wide, so these span a
/// handful of layers (16) up to a deep, wide graph (1024).
const SIZES: [usize; 3] = [16, 128, 1024];
/// Machines each node fans out onto (one [`WorkKey`] per node × machine).
const MACHINES: usize = 4;
/// Nodes per dependency layer; every node depends on the whole previous layer,
/// producing realistic fan-in/fan-out instead of a thin chain.
const WIDTH: usize = 8;

/// A realistic plan/graph fixture: a layered DAG of shell nodes with mixed run
/// policies and plan outcomes, plus the lookup maps the scheduler derives.
struct Fixture {
    infra: Infra,
    plan: Plan,
    cbor: Vec<u8>,
    node_by_id: FxHashMap<NodeId, Node>,
    planned_by_id: FxHashMap<NodeId, PlannedNode>,
    run_policy_by_id: FxHashMap<NodeId, RunPolicy>,
    dependents_by_id: FxHashMap<NodeId, Vec<NodeId>>,
    work: Vec<WorkKey>,
    /// The fixture machine ids (mirrors `infra.machines`); index 0 is used as
    /// the target for the per-machine slice bench.
    machine_ids: Vec<MachineId>,
}

impl Fixture {
    fn build(node_count: usize, machine_count: usize) -> Self {
        // Deterministic ids in a high range so they never collide with node ids.
        let machines: Vec<MachineId> = (0..machine_count)
            .map(|i| MachineId(RawUuid::from_u128(1_000_000 + i as u128)))
            .collect();

        let width = WIDTH.min(node_count.max(1));
        let mut nodes: Vec<Node> = Vec::with_capacity(node_count);
        let mut planned: Vec<PlannedNode> = Vec::with_capacity(node_count);
        let mut work: Vec<WorkKey> = Vec::with_capacity(node_count * machine_count);

        let mut prev_layer: Vec<NodeId> = Vec::new();
        let mut made = 0usize;
        while made < node_count {
            let mut cur_layer: Vec<NodeId> = Vec::new();
            for _ in 0..width {
                if made >= node_count {
                    break;
                }
                made += 1;
                let id = NodeId(RawUuid::from_u128(made as u128));
                // Cycle run policy so demand propagation + change gating both fire.
                let run_policy = match made % 3 {
                    0 => RunPolicy::Lazy,
                    1 => RunPolicy::OnUpstreamChange,
                    _ => RunPolicy::Always,
                };
                let node = NodeBuilder::shell(
                    id,
                    ShellOp::run(vec!["echo".into(), format!("node-{made}")]),
                    Targets::All,
                )
                .name(format!("node-{made}"))
                .deps(prev_layer.clone())
                .run_policy(run_policy)
                .build();
                nodes.push(node);

                // Cycle plan outcomes so should_run / barrier_status see both paths.
                let outcome = match made % 3 {
                    0 => PlanOutcome::Unknown,
                    1 => PlanOutcome::Changed,
                    _ => PlanOutcome::Unchanged,
                };
                planned.push(PlannedNode {
                    node_id: id,
                    name: format!("node-{made}"),
                    description: None,
                    machines: machines.clone(),
                    outcome,
                    fingerprint: Default::default(),
                });
                for &m in &machines {
                    work.push(WorkKey::new(id, m));
                }
                cur_layer.push(id);
            }
            prev_layer = cur_layer;
        }

        let mut infra = Infra::new();
        infra.nodes = nodes.clone();
        // Register the machines so `Targets::All` resolves to a real fan-out for
        // the graph-view and lint benches (slicing reads the planned machines
        // directly and works without this).
        infra.machines = machines
            .iter()
            .enumerate()
            .map(|(i, &id)| Machine {
                id,
                name: format!("machine-{i}"),
                kind: MachineKind::Local,
                vars: VarSet::new(),
                groups: Vec::new(),
                tags: Vec::new(),
                max_parallel_nodes: None,
                lifecycle: Lifecycle::Persistent,
                like: None,
                lazy: false,
            })
            .collect();

        let plan = Plan {
            digest: PlanDigest([0u8; 32]),
            nodes: planned.clone(),
            signatures: vec![],
        };
        let cbor = plan.to_cbor().expect("plan to cbor");

        let node_by_id: FxHashMap<NodeId, Node> = nodes.iter().map(|n| (n.id, n.clone())).collect();
        let planned_by_id: FxHashMap<NodeId, PlannedNode> =
            planned.iter().map(|p| (p.node_id, p.clone())).collect();
        let run_policy_by_id: FxHashMap<NodeId, RunPolicy> =
            nodes.iter().map(|n| (n.id, n.policy.run_policy)).collect();
        let mut dependents_by_id: FxHashMap<NodeId, Vec<NodeId>> = FxHashMap::default();
        for n in &nodes {
            for &dep in &n.deps {
                dependents_by_id.entry(dep).or_default().push(n.id);
            }
        }

        Fixture {
            infra,
            plan,
            cbor,
            node_by_id,
            planned_by_id,
            run_policy_by_id,
            dependents_by_id,
            work,
            machine_ids: machines,
        }
    }

    /// A mid-run decision state: the first half of the units have terminated, and
    /// lazy demand has been propagated — so `next_decisions` exercises the full
    /// blocked / wait / unchanged / start ladder rather than trivially waiting.
    fn mid_run_state(&self) -> GraphState {
        let mut state = GraphState::new();
        let half = self.plan.nodes.len() / 2;
        for (i, p) in self.plan.nodes.iter().take(half).enumerate() {
            let status = if i % 2 == 0 {
                NodeStatus::Changed
            } else {
                NodeStatus::Unchanged
            };
            for &m in &p.machines {
                state.record_completion(WorkKey::new(p.node_id, m), status);
            }
        }
        state.propagate_lazy_demand(
            &self.planned_by_id,
            &self.node_by_id,
            &self.dependents_by_id,
            &self.run_policy_by_id,
        );
        state
    }
}

/// Fingerprinting + digest: CBOR-serialize then SHA-256, once per node and once
/// per whole plan. Runs on every plan write and every drift check.
mod fingerprint {
    use super::*;

    #[divan::bench(args = SIZES)]
    fn node_fingerprints(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        bencher.bench_local(|| {
            let mut acc = 0u8;
            for node in &fx.infra.nodes {
                acc ^= node_fingerprint(node).0[0];
            }
            acc
        });
    }

    #[divan::bench(args = SIZES)]
    fn whole_plan_digest(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        bencher.bench_local(|| plan_digest(&fx.plan));
    }
}

/// Plan (de)serialization on the persistence path (`.plan` file write/read).
mod serialize {
    use super::*;

    #[divan::bench(args = SIZES)]
    fn to_cbor(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        bencher.bench_local(|| fx.plan.to_cbor().expect("to cbor"));
    }

    #[divan::bench(args = SIZES)]
    fn from_cbor(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        bencher.bench_local(|| Plan::from_cbor(&fx.cbor).expect("from cbor"));
    }
}

/// Lowering: `Plan::executable` (maps + fingerprint-drift check) →
/// `ExecutionGraph::from_executable` (target fan-out, body lowering, adjacency) →
/// `to_scheduler_compat` (the legacy lookup maps). This is the per-apply setup.
mod lower {
    use super::*;

    #[divan::bench(args = SIZES)]
    fn plan_executable(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        bencher.bench_local(|| fx.plan.executable(&fx.infra).expect("executable"));
    }

    #[divan::bench(args = SIZES)]
    fn execution_graph_from_executable(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        let exec = fx.plan.executable(&fx.infra).expect("executable");
        bencher.bench_local(|| ExecutionGraph::from_executable(&exec));
    }

    #[divan::bench(args = SIZES)]
    fn to_scheduler_compat(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        let exec = fx.plan.executable(&fx.infra).expect("executable");
        let graph = ExecutionGraph::from_executable(&exec);
        bencher.bench_local(|| graph.to_scheduler_compat());
    }
}

/// The pure decision engine the scheduler calls every tick.
mod decision {
    use super::*;

    /// One full decision sweep over every unit from a mid-run state.
    #[divan::bench(args = SIZES)]
    fn next_decisions(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        let state = fx.mid_run_state();
        bencher.bench_local(|| {
            state.next_decisions(
                &fx.node_by_id,
                &fx.planned_by_id,
                &fx.run_policy_by_id,
                fx.work.iter().copied(),
            )
        });
    }

    /// Lazy-demand back-propagation across the whole graph (fresh state per call).
    #[divan::bench(args = SIZES)]
    fn propagate_lazy_demand(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        bencher
            .with_inputs(GraphState::new)
            .bench_local_values(|mut state| {
                state.propagate_lazy_demand(
                    &fx.planned_by_id,
                    &fx.node_by_id,
                    &fx.dependents_by_id,
                    &fx.run_policy_by_id,
                );
                state
            });
    }
}

/// Whole-infra validation: the single-pass [`Infra::lint_report`] that `plan`
/// runs before anything is written. It rebuilds policy/dependent maps, resolves
/// every node's targets, and walks the graph, capture refs, transports, and
/// plaintext-secret scan — all allocation-heavy and on the critical path.
mod validate {
    use super::*;

    #[divan::bench(args = SIZES)]
    fn lint_report(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        bencher.bench_local(|| fx.infra.lint_report());
    }
}

/// The serializable graph view behind `infrazeug graph` and the dashboards:
/// building it from the infra (target fan-out per node), filtering it to a
/// sub-DAG, and rendering it to text.
mod graph {
    use super::*;

    /// `Infra::graph_view`: resolve every node's targets and emit the node/edge
    /// lists. Runs whenever the graph is rendered or pushed to a watcher.
    #[divan::bench(args = SIZES)]
    fn graph_view(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        bencher.bench_local(|| fx.infra.graph_view().expect("graph view"));
    }

    /// `GraphView::select` with a `start` filter: resolve the node, walk its
    /// descendants, then keep only surviving nodes and their internal edges.
    #[divan::bench(args = SIZES)]
    fn select_descendants(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        let view = fx.infra.graph_view().expect("graph view");
        // `node-1` is in the first layer, so its dependents span the whole DAG.
        let select = GraphSelect {
            start: Some("node-1".into()),
            ..Default::default()
        };
        bencher.bench_local(|| view.select(&select));
    }

    /// `GraphView::to_text`: the default human-readable render (one String).
    #[divan::bench(args = SIZES)]
    fn render_text(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        let view = fx.infra.graph_view().expect("graph view");
        bencher.bench_local(|| view.to_text());
    }
}

/// Per-machine plan slicing on the distribution path: carve the machine's
/// subgraph out of the plan (push mode inserts `WaitForHash` markers for
/// cross-machine deps) and digest the result for integrity.
mod slice {
    use super::*;

    #[divan::bench(args = SIZES)]
    fn slice_for_machine(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        let machine = fx.machine_ids[0];
        bencher.bench_local(|| {
            fx.plan
                .slice_for_machine(&fx.infra, machine, SliceMode::Push)
                .expect("slice")
        });
    }

    #[divan::bench(args = SIZES)]
    fn digest(bencher: Bencher, n: usize) {
        let fx = Fixture::build(n, MACHINES);
        let machine = fx.machine_ids[0];
        let slice: PlanSlice = fx
            .plan
            .slice_for_machine(&fx.infra, machine, SliceMode::Push)
            .expect("slice");
        bencher.bench_local(|| slice_digest(&slice));
    }
}
