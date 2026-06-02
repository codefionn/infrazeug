//! Pure scheduling-decision engine (recommendation 2).
//!
//! [`GraphState`] owns the mutable runtime decision state — per-unit `outcomes`,
//! demand-driven `demanded`/`run_intent` sets — and answers the hard graph
//! questions the scheduler used to compute inline: readiness, upstream blocking,
//! lazy-demand propagation, and run-policy evaluation. It is deliberately free of
//! locks, cancellation, dispatch, and I/O so the SOUL ordering semantics can be
//! tested directly against an [`ExecutionGraph`]-derived view.
//!
//! The scheduler keeps the dispatch concerns (limits, locks, fail-fast,
//! tolerate-cap, cancellation) and calls these methods to decide what is ready.

use crate::execution_graph::WorkKey;
use crate::id::NodeId;
use crate::node::{FailPolicy, Node, NodeStatus, PlanOutcome, RunPolicy};
use crate::plan::PlannedNode;
use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Borrow;

/// Why a unit is skipped instead of started (pure-graph reasons).
///
/// Runtime skip reasons (fail-fast sibling, tolerate-cap exceeded, operator
/// cancel) are dispatch concerns and stay in the scheduler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// An upstream dependency failed (and this node does not tolerate it).
    BlockedByUpstream,
    /// Run policy says this node is unchanged / not pulled, so it is skipped.
    Unchanged,
    /// A lazy node that no live dependent ever demanded.
    NotDemanded,
}

/// A pure-graph decision for a single unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitDecision {
    Start(WorkKey),
    Skip { key: WorkKey, reason: SkipReason },
    Wait,
}

/// Mutable runtime decision state, kept independent of dispatch.
#[derive(Default)]
pub struct GraphState {
    pub outcomes: FxHashMap<WorkKey, NodeStatus>,
    pub demanded: FxHashSet<NodeId>,
    pub run_intent: FxHashSet<NodeId>,
    /// Units skipped because an upstream dependency failed (or was itself
    /// blocked). These are recorded as [`NodeStatus::Skipped`] in `outcomes` for
    /// reporting, but — unlike a benign `unchanged`/`not-demanded` skip — they
    /// produced no output. Tracking them separately lets the block cascade to
    /// dependents (e.g. capture consumers) instead of letting them run against a
    /// capture that was never produced and fail with a cryptic "capture missing".
    pub blocked: FxHashSet<WorkKey>,
}

impl GraphState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of units that have reached a terminal status.
    pub fn completed(&self) -> usize {
        self.outcomes.len()
    }

    pub fn is_decided(&self, key: &WorkKey) -> bool {
        self.outcomes.contains_key(key)
    }

    pub fn outcome(&self, key: &WorkKey) -> Option<NodeStatus> {
        self.outcomes.get(key).copied()
    }

    pub fn is_demanded(&self, node_id: &NodeId) -> bool {
        self.demanded.contains(node_id)
    }

    /// Record a unit's terminal status.
    pub fn record_completion(&mut self, key: WorkKey, status: NodeStatus) {
        self.outcomes.insert(key, status);
    }

    /// Forget a unit's recorded outcome (operator replay).
    pub fn forget(&mut self, key: &WorkKey) -> bool {
        self.blocked.remove(key);
        self.outcomes.remove(key).is_some()
    }

    /// Record that a unit was skipped because an upstream dependency failed (or
    /// was itself blocked). The caller still records the [`NodeStatus::Skipped`]
    /// outcome for reporting; this marks it so dependents cascade-block rather
    /// than treating the skip as a satisfied dependency.
    pub fn mark_blocked(&mut self, key: WorkKey) {
        self.blocked.insert(key);
    }

    pub fn is_blocked(&self, key: &WorkKey) -> bool {
        self.blocked.contains(key)
    }

    /// How many machines of `node_id` have already failed (tolerate-cap input).
    pub fn failed_machines(&self, node_id: NodeId) -> usize {
        self.outcomes
            .iter()
            .filter(|(k, s)| k.node_id == node_id && **s == NodeStatus::Failed)
            .count()
    }

    /// All non-lazy deps reached a terminal status that unblocks this node.
    pub fn deps_satisfied(
        &self,
        node: &Node,
        planned_by_id: &FxHashMap<NodeId, PlannedNode>,
    ) -> bool {
        if tolerates_upstream_failure(node) {
            return node
                .deps
                .iter()
                .all(|dep| upstream_terminal(*dep, planned_by_id, &self.outcomes));
        }
        node.deps
            .iter()
            .all(|dep| upstream_done(*dep, planned_by_id, &self.outcomes, &self.blocked))
    }

    /// An upstream dependency failed and this node does not tolerate it.
    pub fn deps_blocked(
        &self,
        node: &Node,
        planned_by_id: &FxHashMap<NodeId, PlannedNode>,
    ) -> bool {
        if tolerates_upstream_failure(node) {
            return false;
        }
        node.deps
            .iter()
            .any(|dep| upstream_failed(*dep, planned_by_id, &self.outcomes, &self.blocked))
    }

    /// Evaluate run policy: should this (deps-satisfied) node run or skip-unchanged?
    pub fn should_run(
        &self,
        node: &Node,
        planned: &PlannedNode,
        planned_by_id: &FxHashMap<NodeId, PlannedNode>,
        run_policy_by_id: &FxHashMap<NodeId, RunPolicy>,
    ) -> bool {
        if node.body.is_graph_only() {
            return !matches!(node.policy.run_policy, RunPolicy::Lazy)
                || self.demanded.contains(&node.id);
        }

        match node.policy.run_policy {
            RunPolicy::Always => true,
            RunPolicy::Lazy => {
                if !self.demanded.contains(&node.id) {
                    return false;
                }
                let mut strict_deps = node
                    .deps
                    .iter()
                    .filter(|dep| !matches!(run_policy_by_id.get(dep), Some(RunPolicy::Lazy)))
                    .peekable();
                if strict_deps.peek().is_none() {
                    return true;
                }
                planned.outcome == PlanOutcome::Changed
                    || strict_deps
                        .any(|dep| upstream_any_changed(*dep, planned_by_id, &self.outcomes))
            }
            RunPolicy::OnUpstreamChange => {
                if self.run_intent.contains(&node.id) {
                    if lazy_dep_skipped(node, planned_by_id, run_policy_by_id, &self.outcomes) {
                        return false;
                    }
                    return true;
                }
                if planned.outcome == PlanOutcome::Unchanged {
                    return false;
                }
                if node.deps.is_empty() {
                    return true;
                }
                node.deps
                    .iter()
                    .any(|dep| upstream_any_changed(*dep, planned_by_id, &self.outcomes))
                    || planned.outcome == PlanOutcome::Changed
            }
        }
    }

    /// Propagate demand backwards across lazy chains: a lazy node becomes
    /// `demanded` once a live, deps-satisfied, will-run dependent pulls it, and
    /// demand flows transitively to its lazy deps.
    pub fn propagate_lazy_demand<N: Borrow<Node>>(
        &mut self,
        planned_by_id: &FxHashMap<NodeId, PlannedNode>,
        node_by_id: &FxHashMap<NodeId, N>,
        dependents_by_id: &FxHashMap<NodeId, Vec<NodeId>>,
        run_policy_by_id: &FxHashMap<NodeId, RunPolicy>,
    ) {
        let mut newly_demanded = Vec::new();

        for (&candidate, dependents) in dependents_by_id {
            if !planned_by_id.contains_key(&candidate)
                || !matches!(run_policy_by_id.get(&candidate), Some(RunPolicy::Lazy))
            {
                continue;
            }

            for dependent in dependents {
                let Some(node) = node_by_id.get(dependent) else {
                    continue;
                };
                let node = node.borrow();
                let Some(planned) = planned_by_id.get(dependent) else {
                    continue;
                };
                if node_fully_decided(planned, &self.outcomes)
                    || !strict_deps_satisfied(
                        node,
                        planned_by_id,
                        run_policy_by_id,
                        &self.outcomes,
                        &self.blocked,
                    )
                    || !will_run_ignoring_lazy(
                        node,
                        planned,
                        planned_by_id,
                        run_policy_by_id,
                        &self.outcomes,
                        &self.demanded,
                    )
                {
                    continue;
                }
                self.run_intent.insert(*dependent);
                if self.demanded.insert(candidate) {
                    newly_demanded.push(candidate);
                }
            }
        }

        while let Some(node_id) = newly_demanded.pop() {
            let Some(node) = node_by_id.get(&node_id) else {
                continue;
            };
            let node = node.borrow();
            for dep in &node.deps {
                if planned_by_id.contains_key(dep)
                    && matches!(run_policy_by_id.get(dep), Some(RunPolicy::Lazy))
                    && self.demanded.insert(*dep)
                {
                    newly_demanded.push(*dep);
                }
            }
        }
    }

    /// Classify a graph-only barrier's status: `Changed` when its own plan changed
    /// or any upstream changed, else `Unchanged`. Preserves change propagation
    /// through barriers without running a shell command.
    pub fn barrier_status(
        &self,
        node: &Node,
        planned: &PlannedNode,
        planned_by_id: &FxHashMap<NodeId, PlannedNode>,
    ) -> NodeStatus {
        if planned.outcome == PlanOutcome::Changed
            || node
                .deps
                .iter()
                .any(|dep| upstream_any_changed(*dep, planned_by_id, &self.outcomes))
        {
            NodeStatus::Changed
        } else {
            NodeStatus::Unchanged
        }
    }

    /// A lazy node with no live dependent demand: skipped as not-demanded once the
    /// run is otherwise quiescent.
    pub fn is_dormant_lazy(&self, node: &Node) -> bool {
        matches!(node.policy.run_policy, RunPolicy::Lazy) && !self.demanded.contains(&node.id)
    }

    /// Pure-graph decisions for every not-yet-decided unit, used to test the
    /// engine without the dispatch loop. Mirrors the loop's ordering of checks
    /// (blocked → wait-for-deps → unchanged → start); runtime-only skips
    /// (fail-fast, tolerate-cap, cancel) are not modeled here.
    pub fn next_decisions<N: Borrow<Node>>(
        &self,
        node_by_id: &FxHashMap<NodeId, N>,
        planned_by_id: &FxHashMap<NodeId, PlannedNode>,
        run_policy_by_id: &FxHashMap<NodeId, RunPolicy>,
        work: impl Iterator<Item = WorkKey>,
    ) -> Vec<UnitDecision> {
        // Every predicate below (`is_dormant_lazy`, `deps_blocked`,
        // `deps_satisfied`, `should_run`) is node-level: none depend on
        // `key.machine_id`. For a node fanned across M machines the verdict
        // *class* is identical for all M keys; only the emitted `WorkKey`
        // differs. Memoize the class per node so the predicate chain runs once
        // per node instead of once per (node × machine). Output is byte-identical
        // because we still push exactly once per yielded (non-decided,
        // resolvable) key, in iteration order.
        #[derive(Clone, Copy)]
        enum Class {
            Wait,
            Skip(SkipReason),
            Start,
        }
        let mut out = Vec::new();
        let mut verdict_by_node: FxHashMap<NodeId, Class> = FxHashMap::default();
        for key in work {
            if self.outcomes.contains_key(&key) {
                continue;
            }
            let (Some(node), Some(planned)) = (
                node_by_id.get(&key.node_id),
                planned_by_id.get(&key.node_id),
            ) else {
                continue;
            };
            let node = node.borrow();
            let class = *verdict_by_node.entry(key.node_id).or_insert_with(|| {
                if self.is_dormant_lazy(node) {
                    Class::Wait
                } else if self.deps_blocked(node, planned_by_id) {
                    Class::Skip(SkipReason::BlockedByUpstream)
                } else if !self.deps_satisfied(node, planned_by_id) {
                    Class::Wait
                } else if !self.should_run(node, planned, planned_by_id, run_policy_by_id) {
                    Class::Skip(SkipReason::Unchanged)
                } else {
                    Class::Start
                }
            });
            out.push(match class {
                Class::Wait => UnitDecision::Wait,
                Class::Skip(reason) => UnitDecision::Skip { key, reason },
                Class::Start => UnitDecision::Start(key),
            });
        }
        out
    }
}

/// A graph-only join (barrier) marked `Tolerate` waits for every upstream to reach
/// a terminal state — success *or* failure — and runs regardless. Used by dynamic
/// group exit barriers so a failed machine doesn't block the whole fan-out join.
fn tolerates_upstream_failure(node: &Node) -> bool {
    node.body.is_graph_only() && matches!(node.policy.fail_policy, FailPolicy::Tolerate { .. })
}

/// Whether every machine of `dep` has reached any terminal status (including
/// failed/cancelled), as opposed to [`upstream_done`] which requires success.
fn upstream_terminal(
    dep: NodeId,
    planned_by_id: &FxHashMap<NodeId, PlannedNode>,
    outcomes: &FxHashMap<WorkKey, NodeStatus>,
) -> bool {
    let Some(dep_planned) = planned_by_id.get(&dep) else {
        return false;
    };
    dep_planned
        .machines
        .iter()
        .all(|&mid| outcomes.contains_key(&WorkKey::new(dep, mid)))
}

fn node_fully_decided(planned: &PlannedNode, outcomes: &FxHashMap<WorkKey, NodeStatus>) -> bool {
    planned
        .machines
        .iter()
        .all(|&machine_id| outcomes.contains_key(&WorkKey::new(planned.node_id, machine_id)))
}

fn strict_deps_satisfied(
    node: &Node,
    planned_by_id: &FxHashMap<NodeId, PlannedNode>,
    run_policy_by_id: &FxHashMap<NodeId, RunPolicy>,
    outcomes: &FxHashMap<WorkKey, NodeStatus>,
    blocked: &FxHashSet<WorkKey>,
) -> bool {
    node.deps
        .iter()
        .filter(|dep| !matches!(run_policy_by_id.get(dep), Some(RunPolicy::Lazy)))
        .all(|dep| upstream_done(*dep, planned_by_id, outcomes, blocked))
}

fn upstream_done(
    dep: NodeId,
    planned_by_id: &FxHashMap<NodeId, PlannedNode>,
    outcomes: &FxHashMap<WorkKey, NodeStatus>,
    blocked: &FxHashSet<WorkKey>,
) -> bool {
    let Some(dep_planned) = planned_by_id.get(&dep) else {
        return false;
    };
    dep_planned.machines.iter().all(|&mid| {
        let key = WorkKey::new(dep, mid);
        // A unit skipped because it was *blocked* produced no output, so it does
        // not satisfy a dependent — even though its recorded status is `Skipped`.
        !blocked.contains(&key)
            && matches!(
                outcomes.get(&key),
                Some(NodeStatus::Changed | NodeStatus::Unchanged | NodeStatus::Skipped)
            )
    })
}

fn upstream_failed(
    dep: NodeId,
    planned_by_id: &FxHashMap<NodeId, PlannedNode>,
    outcomes: &FxHashMap<WorkKey, NodeStatus>,
    blocked: &FxHashSet<WorkKey>,
) -> bool {
    let Some(dep_planned) = planned_by_id.get(&dep) else {
        return false;
    };
    dep_planned.machines.iter().any(|&mid| {
        let key = WorkKey::new(dep, mid);
        // A blocked upstream propagates like a failure: the dependent can't run
        // because the data/effect it relied on never materialized.
        blocked.contains(&key)
            || matches!(
                outcomes.get(&key),
                Some(NodeStatus::Failed | NodeStatus::Cancelled)
            )
    })
}

fn will_run_ignoring_lazy(
    node: &Node,
    planned: &PlannedNode,
    planned_by_id: &FxHashMap<NodeId, PlannedNode>,
    run_policy_by_id: &FxHashMap<NodeId, RunPolicy>,
    outcomes: &FxHashMap<WorkKey, NodeStatus>,
    demanded: &FxHashSet<NodeId>,
) -> bool {
    match node.policy.run_policy {
        RunPolicy::Always => true,
        RunPolicy::Lazy => demanded.contains(&node.id),
        RunPolicy::OnUpstreamChange => {
            if planned.outcome == PlanOutcome::Unchanged {
                return false;
            }
            let has_lazy_dep = node
                .deps
                .iter()
                .any(|dep| matches!(run_policy_by_id.get(dep), Some(RunPolicy::Lazy)));
            if planned.outcome == PlanOutcome::Unknown && has_lazy_dep {
                return true;
            }
            let mut strict_deps = node
                .deps
                .iter()
                .filter(|dep| !matches!(run_policy_by_id.get(dep), Some(RunPolicy::Lazy)))
                .peekable();
            if strict_deps.peek().is_none() {
                return planned.outcome != PlanOutcome::Unchanged;
            }
            strict_deps.any(|dep| upstream_any_changed(*dep, planned_by_id, outcomes))
                || planned.outcome == PlanOutcome::Changed
        }
    }
}

fn lazy_dep_skipped(
    node: &Node,
    planned_by_id: &FxHashMap<NodeId, PlannedNode>,
    run_policy_by_id: &FxHashMap<NodeId, RunPolicy>,
    outcomes: &FxHashMap<WorkKey, NodeStatus>,
) -> bool {
    node.deps
        .iter()
        .filter(|dep| matches!(run_policy_by_id.get(dep), Some(RunPolicy::Lazy)))
        .any(|dep| upstream_all_skipped(*dep, planned_by_id, outcomes))
}

fn upstream_all_skipped(
    dep: NodeId,
    planned_by_id: &FxHashMap<NodeId, PlannedNode>,
    outcomes: &FxHashMap<WorkKey, NodeStatus>,
) -> bool {
    let Some(dep_planned) = planned_by_id.get(&dep) else {
        return false;
    };
    dep_planned.machines.iter().all(|&mid| {
        matches!(
            outcomes.get(&WorkKey::new(dep, mid)),
            Some(NodeStatus::Skipped)
        )
    })
}

fn upstream_any_changed(
    dep: NodeId,
    planned_by_id: &FxHashMap<NodeId, PlannedNode>,
    outcomes: &FxHashMap<WorkKey, NodeStatus>,
) -> bool {
    let Some(dep_planned) = planned_by_id.get(&dep) else {
        return false;
    };
    dep_planned.machines.iter().any(|&mid| {
        matches!(
            outcomes.get(&WorkKey::new(dep, mid)),
            Some(NodeStatus::Changed)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::MachineId;
    use crate::node::{NodeBuilder, PlanOutcome, Targets};
    use crate::plan::{NodeFingerprint, PlannedNode};
    use infrazeug_shell::ShellOp;
    use uuid::Uuid;

    fn nid() -> NodeId {
        NodeId(Uuid::new_v4())
    }

    fn shell(id: NodeId, deps: Vec<NodeId>, rp: RunPolicy) -> Node {
        NodeBuilder::shell(id, ShellOp::run(vec!["true".into()]), Targets::All)
            .deps(deps)
            .run_policy(rp)
            .build()
    }

    fn planned(id: NodeId, machines: Vec<MachineId>, outcome: PlanOutcome) -> PlannedNode {
        PlannedNode {
            node_id: id,
            name: id.to_string(),
            description: None,
            machines,
            outcome,
            fingerprint: NodeFingerprint::default(),
        }
    }

    /// Build the three lookup maps the engine reads from a list of nodes + plan.
    fn maps(
        nodes: &[Node],
        planned: Vec<PlannedNode>,
    ) -> (
        FxHashMap<NodeId, Node>,
        FxHashMap<NodeId, PlannedNode>,
        FxHashMap<NodeId, RunPolicy>,
        FxHashMap<NodeId, Vec<NodeId>>,
    ) {
        let node_by_id: FxHashMap<NodeId, Node> = nodes.iter().map(|n| (n.id, n.clone())).collect();
        let planned_by_id: FxHashMap<NodeId, PlannedNode> =
            planned.into_iter().map(|p| (p.node_id, p)).collect();
        let run_policy_by_id: FxHashMap<NodeId, RunPolicy> =
            nodes.iter().map(|n| (n.id, n.policy.run_policy)).collect();
        let mut dependents_by_id: FxHashMap<NodeId, Vec<NodeId>> = FxHashMap::default();
        for n in nodes {
            for &dep in &n.deps {
                dependents_by_id.entry(dep).or_default().push(n.id);
            }
        }
        (
            node_by_id,
            planned_by_id,
            run_policy_by_id,
            dependents_by_id,
        )
    }

    #[test]
    fn multi_machine_fanout_waits_for_all_targets() {
        let (m1, m2) = (MachineId(Uuid::new_v4()), MachineId(Uuid::new_v4()));
        let base = nid();
        let dependent = nid();
        let nodes = [
            shell(base, vec![], RunPolicy::OnUpstreamChange),
            shell(dependent, vec![base], RunPolicy::OnUpstreamChange),
        ];
        let (_n, planned_by_id, _rp, _d) = maps(
            &nodes,
            vec![
                planned(base, vec![m1, m2], PlanOutcome::Changed),
                planned(dependent, vec![m1], PlanOutcome::Changed),
            ],
        );

        let mut state = GraphState::new();
        state.record_completion(WorkKey::new(base, m1), NodeStatus::Changed);
        // One of two upstream targets done → dependent not yet satisfied.
        assert!(!state.deps_satisfied(&nodes[1], &planned_by_id));

        state.record_completion(WorkKey::new(base, m2), NodeStatus::Changed);
        assert!(state.deps_satisfied(&nodes[1], &planned_by_id));
    }

    #[test]
    fn failed_upstream_blocks_failfast_dependent() {
        let m1 = MachineId(Uuid::new_v4());
        let base = nid();
        let dependent = nid();
        let nodes = [
            shell(base, vec![], RunPolicy::OnUpstreamChange),
            shell(dependent, vec![base], RunPolicy::OnUpstreamChange),
        ];
        let (_n, planned_by_id, _rp, _d) = maps(
            &nodes,
            vec![
                planned(base, vec![m1], PlanOutcome::Changed),
                planned(dependent, vec![m1], PlanOutcome::Changed),
            ],
        );

        let mut state = GraphState::new();
        state.record_completion(WorkKey::new(base, m1), NodeStatus::Failed);
        assert!(state.deps_blocked(&nodes[1], &planned_by_id));
        assert!(!state.deps_satisfied(&nodes[1], &planned_by_id));
    }

    #[test]
    fn blocked_upstream_cascades_to_capture_consumer() {
        // root --> producer --> consumer (consumer captures producer's output).
        // root fails, so producer is *blocked* (skipped without running). The
        // consumer must cascade-block too rather than treat the skip as satisfied
        // and run against a capture that was never produced.
        let m1 = MachineId(Uuid::new_v4());
        let root = nid();
        let producer = nid();
        let consumer = nid();
        let nodes = [
            shell(root, vec![], RunPolicy::OnUpstreamChange),
            shell(producer, vec![root], RunPolicy::OnUpstreamChange),
            shell(consumer, vec![producer], RunPolicy::OnUpstreamChange),
        ];
        let (_n, planned_by_id, _rp, _d) = maps(
            &nodes,
            vec![
                planned(root, vec![m1], PlanOutcome::Changed),
                planned(producer, vec![m1], PlanOutcome::Changed),
                planned(consumer, vec![m1], PlanOutcome::Changed),
            ],
        );

        let mut state = GraphState::new();
        // root failed → producer is blocked.
        state.record_completion(WorkKey::new(root, m1), NodeStatus::Failed);
        assert!(state.deps_blocked(&nodes[1], &planned_by_id));

        // The scheduler records the producer's skip *and* marks it blocked.
        let producer_key = WorkKey::new(producer, m1);
        state.mark_blocked(producer_key);
        state.record_completion(producer_key, NodeStatus::Skipped);

        // The consumer must be blocked too — not "satisfied" by the producer's skip.
        assert!(state.deps_blocked(&nodes[2], &planned_by_id));
        assert!(!state.deps_satisfied(&nodes[2], &planned_by_id));
    }

    #[test]
    fn benign_unchanged_skip_still_satisfies_dependent() {
        // A producer skipped because it was *unchanged* (not blocked) still
        // satisfies its dependent — only blocked skips cascade.
        let m1 = MachineId(Uuid::new_v4());
        let producer = nid();
        let consumer = nid();
        let nodes = [
            shell(producer, vec![], RunPolicy::OnUpstreamChange),
            shell(consumer, vec![producer], RunPolicy::Always),
        ];
        let (_n, planned_by_id, _rp, _d) = maps(
            &nodes,
            vec![
                planned(producer, vec![m1], PlanOutcome::Unchanged),
                planned(consumer, vec![m1], PlanOutcome::Changed),
            ],
        );

        let mut state = GraphState::new();
        state.record_completion(WorkKey::new(producer, m1), NodeStatus::Skipped);
        assert!(!state.deps_blocked(&nodes[1], &planned_by_id));
        assert!(state.deps_satisfied(&nodes[1], &planned_by_id));
    }

    #[test]
    fn tolerate_graph_only_join_waits_for_terminal_then_runs() {
        let m1 = MachineId(Uuid::new_v4());
        let a = nid();
        let b = nid();
        let exit = nid();
        let exit_node = NodeBuilder::barrier(exit, Targets::All)
            .deps(vec![a, b])
            .fail_policy(FailPolicy::Tolerate {
                max_failed: usize::MAX,
            })
            .build();
        let nodes = [
            shell(a, vec![], RunPolicy::OnUpstreamChange),
            shell(b, vec![], RunPolicy::OnUpstreamChange),
            exit_node,
        ];
        let (_n, planned_by_id, run_policy_by_id, _d) = maps(
            &nodes,
            vec![
                planned(a, vec![m1], PlanOutcome::Changed),
                planned(b, vec![m1], PlanOutcome::Changed),
                planned(exit, vec![m1], PlanOutcome::Unknown),
            ],
        );

        let mut state = GraphState::new();
        state.record_completion(WorkKey::new(a, m1), NodeStatus::Changed);
        // b not terminal yet → tolerate join waits.
        assert!(!state.deps_satisfied(&nodes[2], &planned_by_id));

        state.record_completion(WorkKey::new(b, m1), NodeStatus::Failed);
        // Both terminal: the tolerate join is satisfied, not blocked, and runs.
        assert!(state.deps_satisfied(&nodes[2], &planned_by_id));
        assert!(!state.deps_blocked(&nodes[2], &planned_by_id));
        let exit_planned = &planned_by_id[&exit];
        assert!(state.should_run(&nodes[2], exit_planned, &planned_by_id, &run_policy_by_id));
        // It propagates a changed upstream through the barrier.
        assert_eq!(
            state.barrier_status(&nodes[2], exit_planned, &planned_by_id),
            NodeStatus::Changed
        );
    }

    #[test]
    fn lazy_chain_demand_propagates_through_multiple_deps() {
        let m1 = MachineId(Uuid::new_v4());
        let lazy_a = nid();
        let lazy_b = nid();
        let consumer = nid();
        // consumer (changed) -> lazy_b -> lazy_a, both lazy.
        let nodes = [
            shell(lazy_a, vec![], RunPolicy::Lazy),
            shell(lazy_b, vec![lazy_a], RunPolicy::Lazy),
            shell(consumer, vec![lazy_b], RunPolicy::OnUpstreamChange),
        ];
        let (node_by_id, planned_by_id, run_policy_by_id, dependents_by_id) = maps(
            &nodes,
            vec![
                planned(lazy_a, vec![m1], PlanOutcome::Unknown),
                planned(lazy_b, vec![m1], PlanOutcome::Unknown),
                planned(consumer, vec![m1], PlanOutcome::Changed),
            ],
        );

        let mut state = GraphState::new();
        state.propagate_lazy_demand(
            &planned_by_id,
            &node_by_id,
            &dependents_by_id,
            &run_policy_by_id,
        );
        // Demand reaches the directly-pulled lazy dep and transitively its lazy dep.
        assert!(state.is_demanded(&lazy_b));
        assert!(state.is_demanded(&lazy_a));
    }

    #[test]
    fn lazy_node_without_demand_is_dormant() {
        let m1 = MachineId(Uuid::new_v4());
        let lazy = nid();
        let nodes = [shell(lazy, vec![], RunPolicy::Lazy)];
        let (node_by_id, planned_by_id, run_policy_by_id, dependents_by_id) =
            maps(&nodes, vec![planned(lazy, vec![m1], PlanOutcome::Unknown)]);

        let mut state = GraphState::new();
        state.propagate_lazy_demand(
            &planned_by_id,
            &node_by_id,
            &dependents_by_id,
            &run_policy_by_id,
        );
        assert!(!state.is_demanded(&lazy));
        assert!(state.is_dormant_lazy(&nodes[0]));

        // The pure decision view classifies it as a wait until the run is quiescent.
        let decisions = state.next_decisions(
            &node_by_id,
            &planned_by_id,
            &run_policy_by_id,
            [WorkKey::new(lazy, m1)].into_iter(),
        );
        assert_eq!(decisions, vec![UnitDecision::Wait]);
    }

    #[test]
    fn on_upstream_change_skips_unchanged_starts_changed() {
        let m1 = MachineId(Uuid::new_v4());
        let unchanged = nid();
        let changed = nid();
        let nodes = [
            shell(unchanged, vec![], RunPolicy::OnUpstreamChange),
            shell(changed, vec![], RunPolicy::OnUpstreamChange),
        ];
        let (node_by_id, planned_by_id, run_policy_by_id, _d) = maps(
            &nodes,
            vec![
                planned(unchanged, vec![m1], PlanOutcome::Unchanged),
                planned(changed, vec![m1], PlanOutcome::Changed),
            ],
        );

        let state = GraphState::new();
        assert!(!state.should_run(
            &nodes[0],
            &planned_by_id[&unchanged],
            &planned_by_id,
            &run_policy_by_id
        ));
        assert!(state.should_run(
            &nodes[1],
            &planned_by_id[&changed],
            &planned_by_id,
            &run_policy_by_id
        ));

        // next_decisions: root unchanged → Skip(Unchanged); root changed → Start.
        let decisions = state.next_decisions(
            &node_by_id,
            &planned_by_id,
            &run_policy_by_id,
            [WorkKey::new(unchanged, m1), WorkKey::new(changed, m1)].into_iter(),
        );
        assert!(decisions.contains(&UnitDecision::Skip {
            key: WorkKey::new(unchanged, m1),
            reason: SkipReason::Unchanged,
        }));
        assert!(decisions.contains(&UnitDecision::Start(WorkKey::new(changed, m1))));
    }
}
