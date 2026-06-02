use crate::capture::{resolve_op_captures, CaptureStore};
use crate::decision::GraphState;
use crate::dynamic::DynamicExpansion;
use crate::error::{CoreError, Result};
use crate::events::{SchedCommand, SchedEvent};
use crate::exec::OpExecutor;
use crate::execution_graph::{
    ExecAction, ExecutionGraph, NodeAction, SchedulerCompat, SystemAction, WorkKey,
};
use crate::hash_relay::HashRelay;
use crate::id::{MachineId, NodeId};
use crate::infra::Infra;
use crate::interactor::Interactor;
use crate::limits::GlobalLimits;
use crate::locks::{local_lock_bag, new_local_lock_store, LocalLockStore, LockBag};
use crate::native_exec::NativeExecutor;
use crate::node::{FailPolicy, Node, NodeStatus, NodeSummary, RunPolicy};
use crate::plan::{Plan, PlannedNode};
use crate::report::{RunReport, RunReportEntry};
use crate::retry::ReconnectConfig;
use crate::runtime::VaultSession;
use crate::slice::completion_digest;
use crate::vault_resolve::{resolve_vault_in_shell_op, shell_op_contains_vault};
use async_trait::async_trait;
use futures::FutureExt;
use infrazeug_native::NativeStatus;
use infrazeug_secrets::VaultRef;
use infrazeug_shell::{FileSource, OutputChunk, ShellOp};
use rustc_hash::{FxHashMap, FxHashSet};
use serde_cbor::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

pub struct SchedRuntime<'a> {
    pub infra: &'a Infra,
    pub plan: Plan,
    pub limits: GlobalLimits,
    pub events: tokio::sync::broadcast::Sender<SchedEvent>,
    pub commands: mpsc::Receiver<SchedCommand>,
    pub interact: Arc<dyn Interactor>,
    pub cancel: CancellationToken,
    pub vault: Arc<VaultSession>,
    pub executor: Arc<dyn OpExecutor>,
    pub native_executor: Arc<dyn NativeExecutor>,
    /// Push-mode: report node completion hashes for `WaitForHash` relay.
    pub hash_relay: Option<Arc<HashRelay>>,
    /// Captured node stdout for downstream `WriteFile` sources.
    pub captures: Arc<CaptureStore>,
    /// Spill large captures under this directory (SOUL §3.3.3).
    pub capture_spill_root: Option<PathBuf>,
}

#[async_trait]
pub trait Scheduler: Send + Sync {
    async fn run(&self, runtime: SchedRuntime<'_>) -> Result<RunReport>;
}

pub struct DefaultScheduler;

struct Completion {
    key: WorkKey,
    status: NodeStatus,
    duration: Duration,
    node_name: String,
    message: Option<String>,
}

#[async_trait]
impl Scheduler for DefaultScheduler {
    async fn run(&self, runtime: SchedRuntime<'_>) -> Result<RunReport> {
        let SchedRuntime {
            infra,
            plan,
            limits,
            events,
            mut commands,
            cancel,
            executor,
            native_executor,
            hash_relay,
            captures,
            capture_spill_root,
            ..
        } = runtime;
        let executor = Arc::clone(&executor);
        let native_executor = Arc::clone(&native_executor);
        let captures = Arc::clone(&captures);
        let spill_root = capture_spill_root;

        // Compile the canonical plan into the internal execution IR, then lower it
        // to the scheduler's legacy lookup maps. The maps are owned (not borrowed
        // from infra/plan) so dynamic-group fan-out can insert new per-machine
        // nodes mid-run. See `docs/node-architecture-simplification.md`.
        let exec = plan.executable(infra)?;
        let mut graph = ExecutionGraph::from_executable(&exec);
        drop(exec);
        let SchedulerCompat {
            mut node_by_id,
            mut planned_by_id,
            mut run_policy_by_id,
            mut dependents_by_id,
            mut work,
        } = graph.to_scheduler_compat();

        // Runtime decision state (outcomes + lazy demand) lives in the pure
        // decision engine; the loop owns only dispatch concerns.
        let mut state = GraphState::new();
        let mut machine_permits: HashMap<MachineId, Arc<Semaphore>> = HashMap::new();
        for m in &infra.machines {
            // Clamp low: a zero-permit semaphore would park every unit on this
            // machine forever (acquire() never resolves, no Completion is sent).
            let cap = m.max_parallel_nodes.unwrap_or(64).clamp(1, 64);
            machine_permits.insert(m.id, Arc::new(Semaphore::new(cap)));
        }

        let local_locks: LocalLockStore = new_local_lock_store();
        let global_locks: Arc<Mutex<LockBag>> = Arc::new(Mutex::new(LockBag::default()));
        let node_fail_fast: Arc<Mutex<HashSet<NodeId>>> = Arc::new(Mutex::new(HashSet::new()));
        let successful_by_machine: Arc<Mutex<HashMap<MachineId, Vec<(NodeId, NodeStatus)>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let (done_tx, mut done_rx) = mpsc::channel::<Completion>(work.len().max(8));
        let mut inflight: HashSet<WorkKey> = HashSet::new();
        let mut report = RunReport::default();
        // Discovery nodes already expanded into per-machine work (dynamic groups).
        let mut expanded: HashSet<NodeId> = HashSet::new();

        // Interactive control state (driven by SchedCommand from the TUI, §6ter.7).
        let mut paused = false;
        let mut cancel_work: HashSet<WorkKey> = HashSet::new();
        let mut cancel_machines: HashSet<MachineId> = HashSet::new();
        // Per-unit kill switches for in-flight work (§6ter.5). Child of the run-wide
        // `cancel`, so a global cancel also aborts every running unit.
        let mut inflight_cancel: HashMap<WorkKey, CancellationToken> = HashMap::new();

        let mut planned_by_machine: HashMap<MachineId, usize> = HashMap::new();
        for key in &work {
            *planned_by_machine.entry(key.machine_id).or_insert(0) += 1;
        }
        let planned_by_machine: Vec<(MachineId, usize)> = planned_by_machine.into_iter().collect();
        let machine_by_id: HashMap<MachineId, &crate::machine::Machine> =
            infra.machines.iter().map(|m| (m.id, m)).collect();
        let machine_summaries: Vec<_> = planned_by_machine
            .iter()
            .filter_map(|(id, _)| machine_by_id.get(id).map(|m| (*id, m.summary())))
            .collect();
        let mut node_ids: std::collections::HashSet<NodeId> =
            work.iter().map(|w| w.node_id).collect();
        let mut node_summaries: Vec<(NodeId, crate::node::NodeSummary)> = node_ids
            .drain()
            .map(|id| {
                let summary = node_by_id
                    .get(&id)
                    .map(|n| crate::node::NodeSummary::from_node(n))
                    .or_else(|| {
                        planned_by_id.get(&id).map(|p| crate::node::NodeSummary {
                            name: p.name.clone(),
                            description: p.description.clone(),
                        })
                    })
                    .unwrap_or_else(|| crate::node::NodeSummary {
                        name: id.to_string(),
                        description: None,
                    });
                (id, summary)
            })
            .collect();
        node_summaries.sort_by(|a, b| a.1.name.cmp(&b.1.name));
        let _ = events.send(SchedEvent::RunStarted {
            total_units: work.len(),
            planned_by_machine,
            machine_summaries,
            node_summaries,
        });

        while state.completed() < work.len() {
            if cancel.is_cancelled() {
                break;
            }

            while let Ok(cmd) = commands.try_recv() {
                match cmd {
                    SchedCommand::PauseAll => paused = true,
                    SchedCommand::ResumeAll => paused = false,
                    SchedCommand::CancelNode {
                        node,
                        machine,
                        grace,
                    } => {
                        let key = WorkKey {
                            node_id: node,
                            machine_id: machine,
                        };
                        // In-flight: arm the kill switch after the grace window.
                        // Otherwise mark it so it never starts.
                        if let Some(tok) = inflight_cancel.get(&key) {
                            request_inflight_cancel(&events, node, machine, tok.clone(), grace);
                        } else {
                            cancel_work.insert(key);
                        }
                    }
                    SchedCommand::CancelMachine { machine } => {
                        cancel_machines.insert(machine);
                        // Kill every in-flight unit on this machine with the default grace.
                        for (key, tok) in inflight_cancel.iter() {
                            if key.machine_id == machine {
                                request_inflight_cancel(
                                    &events,
                                    key.node_id,
                                    machine,
                                    tok.clone(),
                                    DEFAULT_CANCEL_GRACE,
                                );
                            }
                        }
                    }
                    SchedCommand::FilterChange { .. } => {
                        // Visual-only on the controller; execution is unaffected (§6ter.7).
                    }
                    SchedCommand::ReplayNode { node, machine } => {
                        // Re-queue a finished unit by clearing its recorded outcome;
                        // the dispatch loop below will pick it up again (§6ter.5).
                        let key = WorkKey {
                            node_id: node,
                            machine_id: machine,
                        };
                        if state.forget(&key) {
                            // Clear the fail-fast mark, or the dispatch pass below
                            // immediately re-skips the replayed unit as its own
                            // "fail-fast sibling" instead of re-running it.
                            node_fail_fast.lock().await.remove(&node);
                            forget_successful_completion(&successful_by_machine, key).await;
                            report
                                .entries
                                .retain(|e| !(e.node_id == node && e.machine_id == machine));
                            let _ = events.send(SchedEvent::NodeQueued { node, machine });
                        }
                    }
                }
            }

            state.propagate_lazy_demand(
                &planned_by_id,
                &node_by_id,
                &dependents_by_id,
                &run_policy_by_id,
            );

            for key in work.iter().copied().collect::<Vec<_>>() {
                if state.is_decided(&key) || inflight.contains(&key) {
                    continue;
                }

                // Honor cancellation requests for not-yet-started work. In-flight
                // units keep running (per-node kill needs RPC grace, §3.8.6).
                if cancel_work.remove(&key) || cancel_machines.contains(&key.machine_id) {
                    finish_cancel(
                        &mut state.outcomes,
                        &mut report,
                        &events,
                        key,
                        &node_by_id[&key.node_id],
                    );
                    continue;
                }

                // While paused, don't start new work — but keep draining commands
                // and collecting completions for in-flight units.
                if paused {
                    continue;
                }

                let node = &node_by_id[&key.node_id];
                let planned = &planned_by_id[&key.node_id];

                if state.is_dormant_lazy(node) {
                    continue;
                }

                if node_fail_fast.lock().await.contains(&key.node_id) {
                    finish_skip(
                        &mut state.outcomes,
                        &mut report,
                        &events,
                        key,
                        node,
                        "fail-fast sibling",
                    );
                    continue;
                }

                if state.deps_blocked(node, &planned_by_id) {
                    // Mark the unit blocked (not merely skipped) so the block
                    // cascades to *its* dependents — e.g. a capture consumer must
                    // not run against a capture an unreachable host never produced.
                    state.mark_blocked(key);
                    finish_skip(
                        &mut state.outcomes,
                        &mut report,
                        &events,
                        key,
                        node,
                        "blocked by upstream",
                    );
                    continue;
                }
                if !state.deps_satisfied(node, &planned_by_id) {
                    continue;
                }

                if !state.should_run(node, planned, &planned_by_id, &run_policy_by_id) {
                    finish_skip(
                        &mut state.outcomes,
                        &mut report,
                        &events,
                        key,
                        node,
                        "unchanged",
                    );
                    continue;
                }

                let barrier_status = if node.body.is_graph_only() {
                    Some(state.barrier_status(node, planned, &planned_by_id))
                } else {
                    None
                };

                if let FailPolicy::Tolerate { max_failed } = node.policy.fail_policy {
                    if state.failed_machines(key.node_id) > max_failed {
                        finish_skip(
                            &mut state.outcomes,
                            &mut report,
                            &events,
                            key,
                            node,
                            "tolerate exceeded",
                        );
                        continue;
                    }
                }

                inflight.insert(key);
                let unit_cancel = cancel.child_token();
                inflight_cancel.insert(key, unit_cancel.clone());
                let events = events.clone();
                let cancel = cancel.clone();
                let local_lock_store = Arc::clone(&local_locks);
                let global_locks = Arc::clone(&global_locks);
                let node_fail_fast = Arc::clone(&node_fail_fast);
                let machine_sem = machine_permits
                    .get(&key.machine_id)
                    .cloned()
                    .unwrap_or_else(|| Arc::new(Semaphore::new(64)));
                let global_node = limits.node_semaphore.clone();
                let node = node.clone();
                let done_tx = done_tx.clone();
                let executor = Arc::clone(&executor);
                let native_executor = Arc::clone(&native_executor);
                let captures = Arc::clone(&captures);
                let spill_root = spill_root.clone();
                let vault = Arc::clone(&runtime.vault);
                let successful_by_machine = Arc::clone(&successful_by_machine);

                tokio::spawn(async move {
                    if cancel.is_cancelled() {
                        let _ = done_tx
                            .send(Completion {
                                key,
                                status: NodeStatus::Cancelled,
                                duration: Duration::ZERO,
                                node_name: node.name.clone(),
                                message: Some("run cancelled".into()),
                            })
                            .await;
                        return;
                    }

                    let _g = global_node.acquire().await.ok();
                    let _m = machine_sem.acquire().await.ok();

                    let local_names = node.policy.locks.local_locks.clone();
                    let global_names = node.policy.locks.global_locks.clone();
                    let machine_id = key.machine_id;
                    let started = Instant::now();
                    let run = async {
                        let _global_guards =
                            LockBag::acquire_named(&global_locks, &global_names).await;
                        let local_bag = local_lock_bag(&local_lock_store, machine_id).await;
                        let _local_guards = LockBag::acquire_named(&local_bag, &local_names).await;
                        let prior_completed =
                            successful_completed_on_machine(&successful_by_machine, machine_id)
                                .await;
                        execute_action(
                            executor.as_ref(),
                            native_executor.as_ref(),
                            &events,
                            &node,
                            machine_id,
                            &captures,
                            spill_root.as_ref(),
                            &vault,
                            &prior_completed,
                            barrier_status,
                        )
                        .await
                    };
                    // A panicking op must still produce a Completion — otherwise
                    // the unit stays inflight forever and the run loop never
                    // terminates. Catch the unwind and report the unit as Failed.
                    let run = std::panic::AssertUnwindSafe(run).catch_unwind();
                    // Race execution against this unit's kill switch (§6ter.5). On
                    // cancel we abort the op future (dropping it) and report Cancelled.
                    let (status, duration, message) = tokio::select! {
                        biased;
                        _ = unit_cancel.cancelled() => {
                            let _ = events.send(SchedEvent::NodeCancelled {
                                node: node.id,
                                machine: key.machine_id,
                                reason: "killed after grace".into(),
                            });
                            (
                                NodeStatus::Cancelled,
                                started.elapsed(),
                                Some("killed after grace".into()),
                            )
                        }
                        res = run => match res {
                            Ok(res) => res,
                            Err(payload) => {
                                let panic_msg = payload
                                    .downcast_ref::<&str>()
                                    .map(|s| (*s).to_string())
                                    .or_else(|| payload.downcast_ref::<String>().cloned())
                                    .unwrap_or_else(|| "unknown panic".into());
                                (
                                    NodeStatus::Failed,
                                    started.elapsed(),
                                    Some(format!("node execution panicked: {panic_msg}")),
                                )
                            }
                        },
                    };

                    if status == NodeStatus::Failed
                        && matches!(node.policy.fail_policy, FailPolicy::FailFast)
                    {
                        node_fail_fast.lock().await.insert(key.node_id);
                    }

                    let _ = done_tx
                        .send(Completion {
                            key,
                            status,
                            duration,
                            node_name: node.name.clone(),
                            message,
                        })
                        .await;
                });
            }

            if inflight.is_empty() {
                if state.completed() >= work.len() {
                    break;
                }
                if !paused {
                    let mut skipped_dormant = false;
                    for key in work.iter().copied().collect::<Vec<_>>() {
                        if state.is_decided(&key) || inflight.contains(&key) {
                            continue;
                        }
                        let node = &node_by_id[&key.node_id];
                        if state.is_dormant_lazy(node) {
                            finish_skip(
                                &mut state.outcomes,
                                &mut report,
                                &events,
                                key,
                                node,
                                "not demanded",
                            );
                            skipped_dormant = true;
                        }
                    }
                    if skipped_dormant {
                        continue;
                    }
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
                continue;
            }

            // Wait for the next completion, but wake periodically so inbound
            // commands (cancel/pause/replay) are drained even while work is
            // in-flight and nothing has finished yet.
            let done = match tokio::time::timeout(Duration::from_millis(50), done_rx.recv()).await {
                Ok(maybe) => maybe,
                Err(_) => continue,
            };
            if let Some(done) = done {
                inflight.remove(&done.key);
                inflight_cancel.remove(&done.key);
                state.record_completion(done.key, done.status);
                record_successful_completion(&successful_by_machine, done.key, done.status).await;
                if matches!(done.status, NodeStatus::Changed | NodeStatus::Unchanged) {
                    if let Some(relay) = &hash_relay {
                        let node = node_by_id.get(&done.key.node_id);
                        if let Some(node) = node {
                            let sources: Vec<MachineId> = planned_by_id
                                .get(&done.key.node_id)
                                .map(|p| p.machines.clone())
                                .unwrap_or_default();
                            let digest = completion_digest(&node.id, &sources);
                            relay
                                .report_node_completion(node.id, &sources, digest)
                                .await;
                        }
                    }
                }
                if let Some(entry) = report
                    .entries
                    .iter_mut()
                    .find(|e| e.node_id == done.key.node_id && e.machine_id == done.key.machine_id)
                {
                    entry.status = done.status;
                    entry.duration = done.duration;
                    if done.message.is_some() {
                        entry.message = done.message.clone();
                    }
                } else {
                    report.entries.push(RunReportEntry {
                        node_id: done.key.node_id,
                        node_name: done.node_name,
                        machine_id: done.key.machine_id,
                        status: done.status,
                        duration: done.duration,
                        message: done.message,
                    });
                }

                // Dynamic-group fan-out: when a discovery node resolves its machines,
                // instantiate the group's template once per machine into new work.
                if matches!(done.status, NodeStatus::Changed | NodeStatus::Unchanged)
                    && infra
                        .dynamic_groups
                        .iter()
                        .any(|g| g.discovery_node == done.key.node_id)
                    && expanded.insert(done.key.node_id)
                {
                    let group = infra
                        .dynamic_groups
                        .iter()
                        .find(|g| g.discovery_node == done.key.node_id)
                        .expect("checked")
                        .clone();
                    if let Err(e) = apply_dynamic_expansion(
                        &group,
                        done.key.machine_id,
                        captures.as_ref(),
                        executor.as_ref(),
                        &events,
                        &mut graph,
                        &mut node_by_id,
                        &mut planned_by_id,
                        &mut run_policy_by_id,
                        &mut dependents_by_id,
                        &mut work,
                        &mut machine_permits,
                    )
                    .await
                    {
                        tracing::error!(group = %group.label, "dynamic expansion failed: {e}");
                    }
                }
            }
        }

        drop(done_tx);

        let succeeded = report
            .entries
            .iter()
            .filter(|e| matches!(e.status, NodeStatus::Changed | NodeStatus::Unchanged))
            .count();
        let failed = report
            .entries
            .iter()
            .filter(|e| e.status == NodeStatus::Failed)
            .count();
        let cancelled = report
            .entries
            .iter()
            .filter(|e| e.status == NodeStatus::Cancelled)
            .count();
        let _ = events.send(SchedEvent::RunFinished {
            total_units: work.len(),
            succeeded,
            failed,
            cancelled,
        });

        Ok(report)
    }
}

/// Expand a dynamic group's template once its discovery node resolves, then splice
/// the result into the live run.
///
/// Template-remapping mechanics live in [`crate::dynamic::compile_expansion`]; the
/// scheduler only reads the discovery capture, registers discovered machines with
/// the executor, applies the returned [`ExecutionGraphPatch`] to the graph,
/// refreshes the legacy lookup maps from it, and emits `UnitsAdded`
/// (recommendation 4).
#[allow(clippy::too_many_arguments)]
async fn apply_dynamic_expansion(
    group: &crate::dynamic::DynamicGroup,
    seed_machine: MachineId,
    captures: &CaptureStore,
    executor: &dyn OpExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    graph: &mut ExecutionGraph,
    node_by_id: &mut FxHashMap<NodeId, Arc<Node>>,
    planned_by_id: &mut FxHashMap<NodeId, PlannedNode>,
    run_policy_by_id: &mut FxHashMap<NodeId, RunPolicy>,
    dependents_by_id: &mut FxHashMap<NodeId, Vec<NodeId>>,
    work: &mut FxHashSet<WorkKey>,
    machine_permits: &mut HashMap<MachineId, Arc<Semaphore>>,
) -> Result<()> {
    let bytes = captures.get(group.discovery_node, seed_machine).await?;
    let DynamicExpansion {
        graph_patch,
        machines_to_register,
    } = crate::dynamic::compile_expansion(group, &bytes, graph)?;

    if graph_patch.units.is_empty() {
        return Ok(());
    }

    // Build the `UnitsAdded` payload from the patch before applying it.
    let added_units = graph_patch.units.len();
    let mut per_machine: HashMap<MachineId, usize> = HashMap::new();
    for unit in &graph_patch.units {
        *per_machine.entry(unit.machine_id).or_insert(0) += 1;
    }
    let planned_by_machine: Vec<(MachineId, usize)> = per_machine.into_iter().collect();
    let machine_summaries: Vec<(MachineId, crate::machine::MachineSummary)> = machines_to_register
        .iter()
        .map(|m| (m.id, m.summary()))
        .collect();
    let node_summaries: Vec<(NodeId, NodeSummary)> = graph_patch
        .nodes
        .iter()
        .map(|n| (n.id, n.summary.clone()))
        .collect();

    // Register discovered machines so the executor can connect lazily on first use.
    for machine in &machines_to_register {
        executor.register_machine(machine.clone()).await;
        machine_permits
            .entry(machine.id)
            .or_insert_with(|| Arc::new(Semaphore::new(64)));
    }

    // Apply to the IR, then refresh the legacy maps the dispatch loop reads from.
    graph.apply_patch(graph_patch);
    let SchedulerCompat {
        node_by_id: new_nodes,
        planned_by_id: new_planned,
        run_policy_by_id: new_run_policy,
        dependents_by_id: new_dependents,
        work: new_work,
    } = graph.to_scheduler_compat();
    *node_by_id = new_nodes;
    *planned_by_id = new_planned;
    *run_policy_by_id = new_run_policy;
    *dependents_by_id = new_dependents;
    *work = new_work;

    let _ = events.send(SchedEvent::UnitsAdded {
        added_units,
        planned_by_machine,
        machine_summaries,
        node_summaries,
    });
    Ok(())
}

fn finish_skip(
    outcomes: &mut FxHashMap<WorkKey, NodeStatus>,
    report: &mut RunReport,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    key: WorkKey,
    node: &Node,
    reason: &str,
) {
    outcomes.insert(key, NodeStatus::Skipped);
    report.entries.push(RunReportEntry {
        node_id: key.node_id,
        node_name: node.name.clone(),
        machine_id: key.machine_id,
        status: NodeStatus::Skipped,
        duration: Duration::ZERO,
        message: Some(reason.into()),
    });
    let _ = events.send(SchedEvent::NodeFinished {
        node: key.node_id,
        machine: key.machine_id,
        status: NodeStatus::Skipped,
        duration: Duration::ZERO,
    });
}

/// Grace before hard-killing in-flight work on a machine-wide cancel (§6ter.5).
const DEFAULT_CANCEL_GRACE: Duration = Duration::from_secs(10);

/// Arm an in-flight unit's kill switch after `grace`. A zero grace cancels
/// immediately; otherwise a detached timer fires the token, letting the op
/// finish politely first (§6ter.5).
fn request_inflight_cancel(
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: NodeId,
    machine: MachineId,
    token: CancellationToken,
    grace: Duration,
) {
    let _ = events.send(SchedEvent::NodeProgress {
        node,
        machine,
        message: format!("cancel requested; killing after {grace:?}"),
    });
    if grace.is_zero() {
        token.cancel();
    } else {
        tokio::spawn(async move {
            tokio::time::sleep(grace).await;
            token.cancel();
        });
    }
}

fn finish_cancel(
    outcomes: &mut FxHashMap<WorkKey, NodeStatus>,
    report: &mut RunReport,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    key: WorkKey,
    node: &Node,
) {
    outcomes.insert(key, NodeStatus::Cancelled);
    report.entries.push(RunReportEntry {
        node_id: key.node_id,
        node_name: node.name.clone(),
        machine_id: key.machine_id,
        status: NodeStatus::Cancelled,
        duration: Duration::ZERO,
        message: Some("cancelled".into()),
    });
    let _ = events.send(SchedEvent::NodeCancelled {
        node: key.node_id,
        machine: key.machine_id,
        reason: "operator cancel".into(),
    });
}

async fn resolve_vault_in_op(op: &ShellOp, vault: &VaultSession) -> Result<ShellOp> {
    if !shell_op_contains_vault(op) {
        return Ok(op.clone());
    }
    let Some(store) = vault.store() else {
        return Err(CoreError::other(
            "vault-backed ShellOp source requires INFRZEUG_VAULT_STORE (unlock data key at apply)",
        ));
    };
    let mut store = store.lock().await;
    resolve_vault_in_shell_op(op.clone(), &mut store).await
}

/// Execute one unit by dispatching on its lowered [`NodeAction`] (recommendation
/// 3). Emits the `NodeQueued`/`NodeStarted` bookend events, routes to the
/// action-specific executor, and returns the unit's `(status, duration, message)`.
///
/// The `Exec` executors own their own `NodeFinished` emission so they can keep the
/// historical behavior where a few inline error paths (vault/capture resolution,
/// controller-vault write, capture spill) return without a `NodeFinished` event.
#[allow(clippy::too_many_arguments)]
async fn execute_action(
    executor: &dyn OpExecutor,
    native_executor: &dyn NativeExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: &Node,
    machine_id: MachineId,
    captures: &CaptureStore,
    spill_root: Option<&PathBuf>,
    vault: &VaultSession,
    prior_completed: &[(NodeId, NodeStatus)],
    barrier_status: Option<NodeStatus>,
) -> (NodeStatus, Duration, Option<String>) {
    let _ = events.send(SchedEvent::NodeQueued {
        node: node.id,
        machine: machine_id,
    });
    let _ = events.send(SchedEvent::NodeStarted {
        node: node.id,
        machine: machine_id,
    });
    let started = Instant::now();
    match NodeAction::from_node(node) {
        NodeAction::Exec(ExecAction::Shell(op)) => {
            execute_shell(
                executor,
                events,
                node,
                machine_id,
                captures,
                spill_root,
                vault,
                prior_completed,
                &op,
                started,
            )
            .await
        }
        NodeAction::Exec(ExecAction::Native { method_id, input }) => {
            execute_native(
                native_executor,
                events,
                node,
                machine_id,
                captures,
                spill_root,
                vault,
                &method_id,
                &input,
                started,
            )
            .await
        }
        NodeAction::System(SystemAction::Connect) => {
            let (status, message) = execute_connect(executor, machine_id).await;
            finish_unit(events, node.id, machine_id, started, status, message)
        }
        NodeAction::Noop(_) => {
            let (status, message) = execute_noop(barrier_status);
            finish_unit(events, node.id, machine_id, started, status, message)
        }
    }
}

/// Emit a unit's `NodeFinished` event and return its `(status, duration, message)`.
fn finish_unit(
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: NodeId,
    machine: MachineId,
    started: Instant,
    status: NodeStatus,
    message: Option<String>,
) -> (NodeStatus, Duration, Option<String>) {
    let duration = started.elapsed();
    let _ = events.send(SchedEvent::NodeFinished {
        node,
        machine,
        status,
        duration,
    });
    (status, duration, message)
}

/// Graph-only no-op: barriers and group bookends carry forward their precomputed
/// barrier status (`Changed` if their plan/upstream changed, else `Unchanged`).
fn execute_noop(barrier_status: Option<NodeStatus>) -> (NodeStatus, Option<String>) {
    (barrier_status.unwrap_or(NodeStatus::Unchanged), None)
}

/// Force the machine's first transport use: a trivial exec lazily brings up the
/// backend (push: arch probe + agent upload + RPC ping; agentless: SSH
/// reachability; local: no-op). Reports `Changed` on success so machine-root
/// successors wired through this head still run each apply.
async fn execute_connect(
    executor: &dyn OpExecutor,
    machine_id: MachineId,
) -> (NodeStatus, Option<String>) {
    let probe = ShellOp::run(vec!["true".into()]);
    match executor.execute(machine_id, &probe).await {
        Ok(out) if out.exit_code == 0 => (NodeStatus::Changed, None),
        Ok(out) => (
            NodeStatus::Failed,
            Some(format!(
                "connectivity probe failed (exit {})",
                out.exit_code
            )),
        ),
        Err(e) => (NodeStatus::Failed, Some(e.to_string())),
    }
}

/// Serializable shell tier: resolve vault/capture sources, handle controller-side
/// vault writes, then run with poll or retry semantics.
#[allow(clippy::too_many_arguments)]
async fn execute_shell(
    executor: &dyn OpExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: &Node,
    machine_id: MachineId,
    captures: &CaptureStore,
    spill_root: Option<&PathBuf>,
    vault: &VaultSession,
    prior_completed: &[(NodeId, NodeStatus)],
    op: &ShellOp,
    started: Instant,
) -> (NodeStatus, Duration, Option<String>) {
    let op = match resolve_vault_in_op(op, vault).await {
        Ok(op) => op,
        Err(e) => {
            let msg = e.to_string();
            let _ = events.send(SchedEvent::NodeProgress {
                node: node.id,
                machine: machine_id,
                message: msg.clone(),
            });
            return (NodeStatus::Failed, started.elapsed(), Some(msg));
        }
    };
    let resolved = match resolve_op_captures(&op, machine_id, captures).await {
        Ok(op) => op,
        Err(e) => {
            let msg = e.to_string();
            let _ = events.send(SchedEvent::NodeProgress {
                node: node.id,
                machine: machine_id,
                message: msg.clone(),
            });
            return (NodeStatus::Failed, started.elapsed(), Some(msg));
        }
    };

    if let Some(result) = run_controller_vault_write(&resolved, vault).await {
        match result {
            Ok((status, message)) => return (status, started.elapsed(), message),
            Err(e) => {
                let msg = e.to_string();
                let _ = events.send(SchedEvent::NodeProgress {
                    node: node.id,
                    machine: machine_id,
                    message: msg.clone(),
                });
                return (NodeStatus::Failed, started.elapsed(), Some(msg));
            }
        }
    }

    let (status, message) = if let Some(poll_cfg) = &node.policy.poll {
        run_poll(executor, events, node, machine_id, poll_cfg, &resolved).await
    } else {
        run_with_retry(
            executor,
            events,
            node,
            machine_id,
            captures,
            spill_root,
            &resolved,
            prior_completed,
        )
        .await
    };
    finish_unit(events, node.id, machine_id, started, status, message)
}

/// Typed native tier: run the method on Local or the push agent, spill any
/// capture, and map the native status onto the graph status.
#[allow(clippy::too_many_arguments)]
async fn execute_native(
    native_executor: &dyn NativeExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: &Node,
    machine_id: MachineId,
    captures: &CaptureStore,
    spill_root: Option<&PathBuf>,
    vault: &VaultSession,
    method_id: &str,
    input: &serde_cbor::Value,
    started: Instant,
) -> (NodeStatus, Duration, Option<String>) {
    // `Local` nodes run on the controller, so hand them the unlocked vault
    // session as a secret source (remote routing drops it).
    let secrets = vault.secret_source_with_captures(Some(captures.clone()));
    let (status, message) = match native_executor
        .execute_native(machine_id, node.id, method_id, input, secrets)
        .await
    {
        Ok(result) => {
            if let Some(msg) = &result.message {
                let _ = events.send(SchedEvent::NodeProgress {
                    node: node.id,
                    machine: machine_id,
                    message: msg.clone(),
                });
            }
            if let Some(capture) = result.capture {
                if let Err(e) = captures.put(node.id, machine_id, capture, spill_root).await {
                    let msg = e.to_string();
                    let _ = events.send(SchedEvent::NodeProgress {
                        node: node.id,
                        machine: machine_id,
                        message: msg.clone(),
                    });
                    return (NodeStatus::Failed, started.elapsed(), Some(msg));
                }
            }
            let status = match result.status {
                NativeStatus::Changed => NodeStatus::Changed,
                NativeStatus::Unchanged => NodeStatus::Unchanged,
            };
            (status, result.message)
        }
        Err(e) => {
            let msg = e.to_string();
            let _ = events.send(SchedEvent::NodeProgress {
                node: node.id,
                machine: machine_id,
                message: msg.clone(),
            });
            (NodeStatus::Failed, Some(msg))
        }
    };
    finish_unit(events, node.id, machine_id, started, status, message)
}

async fn run_controller_vault_write(
    op: &ShellOp,
    vault: &VaultSession,
) -> Option<Result<(NodeStatus, Option<String>)>> {
    if !is_controller_vault_op(op) {
        return None;
    }
    Some(execute_controller_vault_op(op, vault).await)
}

/// Vault ops (and `Seq`s composed purely of them) never reach a shell executor —
/// the controller runs them against the unlocked vault store. A `Seq` mixing
/// vault and non-vault steps is not intercepted and fails in the executor.
fn is_controller_vault_op(op: &ShellOp) -> bool {
    match op {
        ShellOp::VaultWrite { .. } | ShellOp::VaultEnsurePasswordHash { .. } => true,
        ShellOp::Seq { steps } => !steps.is_empty() && steps.iter().all(is_controller_vault_op),
        _ => false,
    }
}

fn execute_controller_vault_op<'a>(
    op: &'a ShellOp,
    vault: &'a VaultSession,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<(NodeStatus, Option<String>)>> + Send + 'a>,
> {
    Box::pin(async move {
        match op {
            ShellOp::VaultWrite {
                data_key_id,
                file,
                field,
                value,
                if_absent,
            } => write_vault_field(data_key_id, file, field, value, *if_absent, vault).await,
            ShellOp::VaultEnsurePasswordHash {
                data_key_id,
                file,
                password_field,
                hash_field,
                password,
                hash,
            } => {
                ensure_vault_password_hash(
                    data_key_id,
                    file,
                    password_field,
                    hash_field,
                    password,
                    hash,
                    vault,
                )
                .await
            }
            ShellOp::Seq { steps } => {
                let mut changed = false;
                let mut messages = Vec::new();
                for step in steps {
                    let (status, message) = execute_controller_vault_op(step, vault).await?;
                    changed |= status == NodeStatus::Changed;
                    if let Some(message) = message {
                        messages.push(message);
                    }
                }
                let status = if changed {
                    NodeStatus::Changed
                } else {
                    NodeStatus::Unchanged
                };
                let message = if messages.is_empty() {
                    None
                } else {
                    Some(messages.join("; "))
                };
                Ok((status, message))
            }
            _ => Err(CoreError::other(
                "internal: execute_controller_vault_op on non-vault op",
            )),
        }
    })
}

async fn write_vault_field(
    data_key_id: &str,
    file: &str,
    field: &str,
    value: &FileSource,
    if_absent: bool,
    vault: &VaultSession,
) -> Result<(NodeStatus, Option<String>)> {
    let Some(store) = vault.store() else {
        return Err(CoreError::other(
            "VaultWrite requires INFRZEUG_VAULT_STORE (unlock data key at apply)",
        ));
    };
    let FileSource::Bytes(bytes) = value else {
        return Err(CoreError::other(
            "VaultWrite value must resolve to bytes before controller execution",
        ));
    };
    // An empty value means the source had nothing to store — e.g. an optional
    // json-pointer (`FileSource::json_pointer_optional`) whose field was absent
    // on this run (a secret only returned once at creation). Leave the existing
    // vault field untouched rather than overwriting it with an empty value.
    if bytes.is_empty() {
        return Ok((
            NodeStatus::Unchanged,
            Some(format!(
                "vault field {file}:{field} left unchanged (source empty)"
            )),
        ));
    }
    let value = bytes_to_vault_value(bytes);
    let mut fields = std::collections::BTreeMap::new();
    fields.insert(field.to_string(), value);
    let mut store = store.lock().await;
    let changed = if if_absent {
        store
            .put_vault_fields_if_absent(data_key_id, file, &fields)
            .await?
    } else {
        store.put_vault_fields(data_key_id, file, &fields).await?
    };
    if !changed {
        let reason = if if_absent {
            "already exists"
        } else {
            "already up-to-date"
        };
        return Ok((
            NodeStatus::Unchanged,
            Some(format!("vault field {file}:{field} {reason}")),
        ));
    }
    Ok((
        NodeStatus::Changed,
        Some(format!("stored vault field {file}:{field}")),
    ))
}

async fn ensure_vault_password_hash(
    data_key_id: &str,
    file: &str,
    password_field: &str,
    hash_field: &str,
    password_spec: &infrazeug_shell::RandomPasswordSpec,
    hash_spec: &infrazeug_shell::PasswordHashSpec,
    vault: &VaultSession,
) -> Result<(NodeStatus, Option<String>)> {
    let Some(store) = vault.store() else {
        return Err(CoreError::other(
            "VaultEnsurePasswordHash requires INFRZEUG_VAULT_STORE (unlock data key at apply)",
        ));
    };
    let mut store = store.lock().await;
    let password_ref = VaultRef::field(file, password_field);
    let hash_ref = VaultRef::field(file, hash_field);
    let current_password = store.resolve_field_optional(&password_ref).await?;
    let current_hash = store.resolve_field_optional(&hash_ref).await?;

    if current_password.is_some() && current_hash.is_some() {
        return Ok((
            NodeStatus::Unchanged,
            Some(format!(
                "vault password/hash fields {file}:{password_field},{hash_field} already exist"
            )),
        ));
    }

    let password = match current_password {
        Some(value) => crate::vault_resolve::vault_value_to_bytes(value)?,
        None => match infrazeug_shell::resolve::resolve_literal_file_source(
            &FileSource::random_password(password_spec.clone()),
        )
        .map_err(|e| CoreError::other(e.to_string()))?
        {
            FileSource::Bytes(bytes) => bytes,
            _ => return Err(CoreError::other("random password did not resolve to bytes")),
        },
    };

    let mut fields = std::collections::BTreeMap::new();
    if store.resolve_field_optional(&password_ref).await?.is_none() {
        fields.insert(password_field.to_string(), bytes_to_vault_value(&password));
    }
    if current_hash.is_none() {
        let hash = infrazeug_shell::resolve::hash_password(&password, hash_spec)
            .map_err(|e| CoreError::other(e.to_string()))?;
        fields.insert(hash_field.to_string(), bytes_to_vault_value(&hash));
    }

    if fields.is_empty() {
        return Ok((
            NodeStatus::Unchanged,
            Some(format!(
                "vault password/hash fields {file}:{password_field},{hash_field} already exist"
            )),
        ));
    }

    let changed = store
        .put_vault_fields_if_absent(data_key_id, file, &fields)
        .await?;
    if !changed {
        return Ok((
            NodeStatus::Unchanged,
            Some(format!(
                "vault password/hash fields {file}:{password_field},{hash_field} already exist"
            )),
        ));
    }
    Ok((
        NodeStatus::Changed,
        Some(format!(
            "stored missing vault password/hash fields {file}:{password_field},{hash_field}"
        )),
    ))
}

fn bytes_to_vault_value(bytes: &[u8]) -> Value {
    match std::str::from_utf8(bytes) {
        Ok(text) => Value::Text(text.to_string()),
        Err(_) => Value::Bytes(bytes.to_vec()),
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_with_retry(
    executor: &dyn OpExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: &Node,
    machine_id: MachineId,
    captures: &CaptureStore,
    spill_root: Option<&PathBuf>,
    resolved: &ShellOp,
    prior_completed: &[(NodeId, NodeStatus)],
) -> (NodeStatus, Option<String>) {
    let retry_cfg = &node.policy.retry;
    let idempotent = is_idempotent(resolved);
    let mut attempt: u32 = 0;
    let reconnect_cfg = ReconnectConfig::default();

    // Capture the host's boot id *before* a shutdown-causing op runs, so the
    // reboot can later be proven real (the boot id will differ) rather than
    // accepting a transport that merely reconnected to a still-live host.
    let pre_boot_id = if node.expects_reboot() {
        probe_boot_id(executor, machine_id).await
    } else {
        None
    };

    loop {
        match execute_with_progress(executor, events, node, machine_id, resolved).await {
            Ok(out) => {
                let _ = events.send(SchedEvent::NodeProgress {
                    node: node.id,
                    machine: machine_id,
                    message: format!(
                        "exit={} stdout={} stderr={}",
                        out.exit_code,
                        out.stdout.len(),
                        out.stderr.len()
                    ),
                });
                if out.exit_code == 0 {
                    // Classify before expect_shutdown handling. A command that
                    // proves it was a no-op must not enter reboot/reconnect
                    // handling or propagate change to dependent nodes.
                    let classified = classify_shell_success(node, &out);
                    if let Err(e) = captures
                        .put(node.id, machine_id, out.stdout, spill_root)
                        .await
                    {
                        let msg = e.to_string();
                        let _ = events.send(SchedEvent::NodeProgress {
                            node: node.id,
                            machine: machine_id,
                            message: msg.clone(),
                        });
                        return (NodeStatus::Failed, Some(msg));
                    }
                    if classified == NodeStatus::Unchanged {
                        return (NodeStatus::Unchanged, None);
                    }
                    if node.expects_reboot() {
                        // Clean exit 0: the host is still up (async reboot just
                        // triggered), so no disconnect has been observed yet.
                        return handle_expected_shutdown(
                            executor,
                            events,
                            node,
                            machine_id,
                            prior_completed,
                            &pre_boot_id,
                            false,
                        )
                        .await;
                    }
                    return (classified, None);
                }
                let msg = shell_failure_message(out.exit_code, &out.stderr, &out.stdout);
                if node.expects_reboot() {
                    // Non-zero exit on a reboot node usually means the connection
                    // was torn down mid-command — treat as an observed disconnect.
                    return handle_expected_shutdown(
                        executor,
                        events,
                        node,
                        machine_id,
                        prior_completed,
                        &pre_boot_id,
                        true,
                    )
                    .await;
                }
                if !retry_cfg.should_retry(idempotent, attempt) {
                    return (NodeStatus::Failed, Some(msg));
                }
                let _ = events.send(SchedEvent::NodeRetrying {
                    node: node.id,
                    machine: machine_id,
                    attempt: attempt + 1,
                    max_attempts: retry_cfg.max_attempts,
                    message: msg,
                });
                retry_cfg.wait_before_retry(attempt).await;
                attempt += 1;
            }
            Err(e) => {
                let msg = e.to_string();
                let _ = events.send(SchedEvent::NodeProgress {
                    node: node.id,
                    machine: machine_id,
                    message: msg.clone(),
                });

                if node.expects_reboot() {
                    // The op errored — the transport dropped (the reboot took the
                    // connection with it). That counts as an observed disconnect.
                    return handle_expected_shutdown(
                        executor,
                        events,
                        node,
                        machine_id,
                        prior_completed,
                        &pre_boot_id,
                        true,
                    )
                    .await;
                }

                if retry_cfg.should_retry(idempotent, attempt) {
                    if let Err(reconnect_msg) = wait_for_reconnect(
                        executor,
                        events,
                        node,
                        machine_id,
                        reconnect_cfg,
                        None,
                        "retry",
                    )
                    .await
                    {
                        let _ = events.send(SchedEvent::NodeProgress {
                            node: node.id,
                            machine: machine_id,
                            message: reconnect_msg,
                        });
                    }
                    let _ = events.send(SchedEvent::NodeRetrying {
                        node: node.id,
                        machine: machine_id,
                        attempt: attempt + 1,
                        max_attempts: retry_cfg.max_attempts,
                        message: msg.clone(),
                    });
                    retry_cfg.wait_before_retry(attempt).await;
                    attempt += 1;
                } else {
                    return (NodeStatus::Failed, Some(msg));
                }
            }
        }
    }
}

#[allow(clippy::type_complexity)]
async fn successful_completed_on_machine(
    completed_by_machine: &Arc<Mutex<HashMap<MachineId, Vec<(NodeId, NodeStatus)>>>>,
    machine_id: MachineId,
) -> Vec<(NodeId, NodeStatus)> {
    let mut completed = completed_by_machine
        .lock()
        .await
        .get(&machine_id)
        .cloned()
        .unwrap_or_default();
    completed.sort_by_key(|(node_id, _)| *node_id);
    completed
}

#[allow(clippy::type_complexity)]
async fn record_successful_completion(
    completed_by_machine: &Arc<Mutex<HashMap<MachineId, Vec<(NodeId, NodeStatus)>>>>,
    key: WorkKey,
    status: NodeStatus,
) {
    if !matches!(status, NodeStatus::Changed | NodeStatus::Unchanged) {
        return;
    }
    let mut by_machine = completed_by_machine.lock().await;
    let completed = by_machine.entry(key.machine_id).or_default();
    if let Some((_, existing_status)) = completed
        .iter_mut()
        .find(|(node_id, _)| *node_id == key.node_id)
    {
        *existing_status = status;
    } else {
        completed.push((key.node_id, status));
    }
}

#[allow(clippy::type_complexity)]
async fn forget_successful_completion(
    completed_by_machine: &Arc<Mutex<HashMap<MachineId, Vec<(NodeId, NodeStatus)>>>>,
    key: WorkKey,
) {
    let mut by_machine = completed_by_machine.lock().await;
    if let Some(completed) = by_machine.get_mut(&key.machine_id) {
        completed.retain(|(node_id, _)| *node_id != key.node_id);
    }
}

async fn sync_reconnected_node_graph_state(
    executor: &dyn OpExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: &Node,
    machine_id: MachineId,
    prior_completed: &[(NodeId, NodeStatus)],
) -> std::result::Result<(), String> {
    let mut completed = prior_completed.to_vec();
    completed.push((node.id, NodeStatus::Changed));

    let _ = events.send(SchedEvent::NodeProgress {
        node: node.id,
        machine: machine_id,
        message: format!(
            "expect_shutdown: syncing {} completed graph nodes after reconnect",
            completed.len()
        ),
    });

    executor
        .sync_node_graph_state(machine_id, &completed)
        .await
        .map_err(|e| format!("sync node graph state after reconnect failed: {e}"))
}

async fn handle_expected_shutdown(
    executor: &dyn OpExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: &Node,
    machine_id: MachineId,
    prior_completed: &[(NodeId, NodeStatus)],
    pre_boot_id: &Option<String>,
    // True when the op already errored / closed the connection (a synchronous
    // reboot), which counts as an observed disconnect for the no-baseline path.
    disconnect_observed: bool,
) -> (NodeStatus, Option<String>) {
    let _ = events.send(SchedEvent::NodeProgress {
        node: node.id,
        machine: machine_id,
        message: "expect_shutdown: waiting for host to reboot".into(),
    });
    match wait_for_reboot(
        executor,
        events,
        node,
        machine_id,
        pre_boot_id,
        disconnect_observed,
        ReconnectConfig::reboot_default(),
    )
    .await
    {
        Ok(()) => match sync_reconnected_node_graph_state(
            executor,
            events,
            node,
            machine_id,
            prior_completed,
        )
        .await
        {
            Ok(()) => (NodeStatus::Changed, None),
            Err(msg) => (NodeStatus::Failed, Some(msg)),
        },
        Err(msg) => (NodeStatus::Failed, Some(msg)),
    }
}

/// Linux boot id (`/proc/sys/kernel/random/boot_id`): a fresh random value on
/// every boot, so a change proves the host actually rebooted.
fn boot_id_probe() -> ShellOp {
    ShellOp::run(vec!["cat".into(), "/proc/sys/kernel/random/boot_id".into()])
}

/// Best-effort read of the current boot id; `None` if unavailable (non-Linux,
/// transient failure), in which case the reboot is confirmed by an observed
/// disconnect instead.
async fn probe_boot_id(executor: &dyn OpExecutor, machine_id: MachineId) -> Option<String> {
    match executor.execute(machine_id, &boot_id_probe()).await {
        Ok(out) if out.exit_code == 0 => {
            let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
            (!id.is_empty()).then_some(id)
        }
        _ => None,
    }
}

/// Wait for an `expect_shutdown` host to actually reboot and become ready again.
///
/// Two phases: (1) confirm a *real* reboot — the boot id must differ from the
/// pre-reboot baseline (or, with no baseline, the transport must have actually
/// dropped) before a reconnect is accepted; (2) if the node carries a
/// [`readiness_check`](Node::readiness_check), poll it until it exits `0`.
async fn wait_for_reboot(
    executor: &dyn OpExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: &Node,
    machine_id: MachineId,
    pre_boot_id: &Option<String>,
    disconnect_observed: bool,
    cfg: ReconnectConfig,
) -> std::result::Result<(), String> {
    confirm_reboot(
        executor,
        events,
        node,
        machine_id,
        pre_boot_id,
        disconnect_observed,
        &cfg,
    )
    .await?;
    if let Some(check) = node.readiness_check() {
        wait_for_readiness(executor, events, node, machine_id, check, &cfg).await?;
    }
    Ok(())
}

async fn confirm_reboot(
    executor: &dyn OpExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: &Node,
    machine_id: MachineId,
    pre_boot_id: &Option<String>,
    disconnect_observed: bool,
    cfg: &ReconnectConfig,
) -> std::result::Result<(), String> {
    let probe = boot_id_probe();
    let mut attempt: u32 = 0;
    let mut disconnect_observed = disconnect_observed;
    let mut last_error = String::new();

    while cfg.should_reconnect(attempt) {
        let _ = events.send(SchedEvent::NodeReconnecting {
            node: node.id,
            machine: machine_id,
            attempt: attempt + 1,
            message: "expect_shutdown: waiting for host to reboot".into(),
        });

        match executor.reconnect(machine_id).await {
            Ok(()) => match executor.execute(machine_id, &probe).await {
                Ok(out) if out.exit_code == 0 => {
                    let live = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    match pre_boot_id {
                        // Baseline available: only a *different* boot id proves
                        // the reboot. An equal id means we reconnected to a host
                        // that has not gone down yet — keep waiting (the race the
                        // old bare-reconnect path fell into).
                        Some(pre) if !live.is_empty() => {
                            if &live != pre {
                                let _ = events.send(SchedEvent::NodeProgress {
                                    node: node.id,
                                    machine: machine_id,
                                    message: "expect_shutdown: reboot confirmed (boot id changed)"
                                        .into(),
                                });
                                return Ok(());
                            }
                            last_error = "host still up (boot id unchanged)".into();
                        }
                        // No baseline: fall back to requiring that the transport
                        // actually dropped at least once before accepting it.
                        _ => {
                            if disconnect_observed {
                                let _ = events.send(SchedEvent::NodeProgress {
                                    node: node.id,
                                    machine: machine_id,
                                    message:
                                        "expect_shutdown: reboot confirmed (reconnected after disconnect)"
                                            .into(),
                                });
                                return Ok(());
                            }
                            last_error = "no boot-id baseline; awaiting observed disconnect".into();
                        }
                    }
                }
                Ok(out) => {
                    last_error = shell_failure_message(out.exit_code, &out.stderr, &out.stdout);
                }
                Err(e) => {
                    disconnect_observed = true;
                    last_error = e.to_string();
                }
            },
            Err(e) => {
                disconnect_observed = true;
                last_error = e.to_string();
            }
        }

        let _ = events.send(SchedEvent::NodeProgress {
            node: node.id,
            machine: machine_id,
            message: format!("expect_shutdown: not back yet: {last_error}"),
        });

        attempt += 1;
        let delay = cfg.delay_for_attempt(attempt);
        if delay > Duration::ZERO {
            tokio::time::sleep(delay).await;
        }
    }

    Err(format!(
        "expect_shutdown: host did not return from reboot: {}",
        if last_error.is_empty() {
            "attempts exhausted".to_string()
        } else {
            last_error
        }
    ))
}

async fn wait_for_readiness(
    executor: &dyn OpExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: &Node,
    machine_id: MachineId,
    check: &ShellOp,
    cfg: &ReconnectConfig,
) -> std::result::Result<(), String> {
    let mut attempt: u32 = 0;
    let mut last_error = String::new();

    while cfg.should_reconnect(attempt) {
        match executor.execute(machine_id, check).await {
            Ok(out) if out.exit_code == 0 => {
                let _ = events.send(SchedEvent::NodeProgress {
                    node: node.id,
                    machine: machine_id,
                    message: "expect_shutdown: readiness check passed".into(),
                });
                return Ok(());
            }
            Ok(out) => {
                last_error = shell_failure_message(out.exit_code, &out.stderr, &out.stdout);
            }
            Err(e) => {
                // The connection can flap while the node is still coming up.
                last_error = e.to_string();
                let _ = executor.reconnect(machine_id).await;
            }
        }

        let _ = events.send(SchedEvent::NodeProgress {
            node: node.id,
            machine: machine_id,
            message: format!("expect_shutdown: not ready yet: {last_error}"),
        });

        attempt += 1;
        let delay = cfg.delay_for_attempt(attempt);
        if delay > Duration::ZERO {
            tokio::time::sleep(delay).await;
        }
    }

    Err(format!(
        "expect_shutdown: readiness check did not pass: {}",
        if last_error.is_empty() {
            "attempts exhausted".to_string()
        } else {
            last_error
        }
    ))
}

async fn run_poll(
    executor: &dyn OpExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: &Node,
    machine_id: MachineId,
    poll_cfg: &crate::retry::PollConfig,
    resolved: &ShellOp,
) -> (NodeStatus, Option<String>) {
    let reconnect_cfg = ReconnectConfig::reboot_default();
    let _ = events.send(SchedEvent::NodePolling {
        node: node.id,
        machine: machine_id,
        message: format!(
            "polling every {:?} up to {:?}",
            poll_cfg.every, poll_cfg.timeout
        ),
    });
    let deadline = tokio::time::Instant::now() + poll_cfg.timeout;
    loop {
        let result = execute_with_progress(executor, events, node, machine_id, resolved).await;
        match result {
            Ok(out) => {
                if out.exit_code == 0 {
                    // Poll success can still be a no-op if the node policy
                    // recognizes the final check output as unchanged.
                    let status = classify_shell_success(node, &out);
                    let _ = events.send(SchedEvent::NodePolling {
                        node: node.id,
                        machine: machine_id,
                        message: "poll succeeded".into(),
                    });
                    return (status, None);
                }
                let _ = events.send(SchedEvent::NodePolling {
                    node: node.id,
                    machine: machine_id,
                    message: format!("poll check exit={}", out.exit_code),
                });
            }
            Err(e) => {
                let _ = events.send(SchedEvent::NodePolling {
                    node: node.id,
                    machine: machine_id,
                    message: format!("poll check error: {e}"),
                });
                if let Err(msg) = wait_for_reconnect(
                    executor,
                    events,
                    node,
                    machine_id,
                    reconnect_cfg,
                    Some(deadline),
                    "poll",
                )
                .await
                {
                    let _ = events.send(SchedEvent::NodePolling {
                        node: node.id,
                        machine: machine_id,
                        message: msg.clone(),
                    });
                    return (NodeStatus::Failed, Some(msg));
                }
                continue;
            }
        }
        if tokio::time::Instant::now() + poll_cfg.every > deadline {
            let msg = format!("poll timed out after {:?}", poll_cfg.timeout);
            let _ = events.send(SchedEvent::NodePolling {
                node: node.id,
                machine: machine_id,
                message: msg.clone(),
            });
            return (NodeStatus::Failed, Some(msg));
        }
        tokio::time::sleep(poll_cfg.every).await;
    }
}

async fn wait_for_reconnect(
    executor: &dyn OpExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: &Node,
    machine_id: MachineId,
    reconnect_cfg: ReconnectConfig,
    deadline: Option<tokio::time::Instant>,
    context: &str,
) -> std::result::Result<(), String> {
    let health_check = ShellOp::run(vec!["true".into()]);
    let mut reconnect_attempt: u32 = 0;
    let mut last_error = String::new();

    while reconnect_cfg.should_reconnect(reconnect_attempt) {
        if deadline.is_some_and(|d| tokio::time::Instant::now() >= d) {
            break;
        }

        let _ = events.send(SchedEvent::NodeReconnecting {
            node: node.id,
            machine: machine_id,
            attempt: reconnect_attempt + 1,
            message: context.into(),
        });

        match executor.reconnect(machine_id).await {
            Ok(()) => match executor.execute(machine_id, &health_check).await {
                Ok(out) if out.exit_code == 0 => {
                    let _ = events.send(SchedEvent::NodeProgress {
                        node: node.id,
                        machine: machine_id,
                        message: format!("{context}: transport reconnected"),
                    });
                    return Ok(());
                }
                Ok(out) => {
                    last_error = shell_failure_message(out.exit_code, &out.stderr, &out.stdout);
                }
                Err(e) => {
                    last_error = e.to_string();
                }
            },
            Err(e) => {
                last_error = e.to_string();
            }
        }

        let _ = events.send(SchedEvent::NodeProgress {
            node: node.id,
            machine: machine_id,
            message: format!("{context}: reconnect failed: {last_error}"),
        });

        reconnect_attempt += 1;
        let mut delay = reconnect_cfg.delay_for_attempt(reconnect_attempt);
        if let Some(deadline) = deadline {
            let now = tokio::time::Instant::now();
            if now >= deadline {
                break;
            }
            delay = delay.min(deadline.saturating_duration_since(now));
        }
        if delay > Duration::ZERO {
            tokio::time::sleep(delay).await;
        }
    }

    if last_error.is_empty() {
        last_error = "reconnect attempts exhausted".into();
    }
    Err(format!("{context}: reconnect failed: {last_error}"))
}

async fn execute_with_progress(
    executor: &dyn OpExecutor,
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: &Node,
    machine_id: MachineId,
    op: &ShellOp,
) -> infrazeug_shell::Result<infrazeug_shell::local::ExecOutput> {
    let (tx, mut rx) = mpsc::unbounded_channel::<OutputChunk>();
    let fut = executor.execute_streaming(machine_id, op, Some(tx));
    tokio::pin!(fut);
    let mut rx_open = true;

    loop {
        tokio::select! {
            maybe = rx.recv(), if rx_open => {
                if let Some(chunk) = maybe {
                    emit_output_chunk(events, node.id, machine_id, chunk);
                } else {
                    rx_open = false;
                }
            }
            result = &mut fut => {
                while let Ok(chunk) = rx.try_recv() {
                    emit_output_chunk(events, node.id, machine_id, chunk);
                }
                return result;
            }
        }
    }
}

fn emit_output_chunk(
    events: &tokio::sync::broadcast::Sender<SchedEvent>,
    node: NodeId,
    machine: MachineId,
    chunk: OutputChunk,
) {
    if chunk.data.is_empty() {
        return;
    }
    let _ = events.send(SchedEvent::NodeOutput {
        node,
        machine,
        stream: chunk.stream,
        data: chunk.data,
    });
}

fn classify_shell_success(node: &Node, out: &infrazeug_shell::local::ExecOutput) -> NodeStatus {
    match node
        .policy
        .success
        .change_policy
        .classify(&out.stdout, &out.stderr)
    {
        crate::node::OutputMatchStatus::Changed => NodeStatus::Changed,
        crate::node::OutputMatchStatus::Unchanged => NodeStatus::Unchanged,
    }
}

fn is_idempotent(op: &ShellOp) -> bool {
    match op {
        ShellOp::ReadFile { .. } => true,
        ShellOp::EnsureDir { .. } => true,
        ShellOp::SyncDir { .. } => true,
        ShellOp::Seq { steps } => steps.iter().all(is_idempotent),
        ShellOp::Poll { .. } => true,
        ShellOp::Run { .. } => false,
        ShellOp::WriteFile { .. } => false,
        ShellOp::VaultWrite { .. } => false,
        ShellOp::VaultEnsurePasswordHash { .. } => false,
    }
}

fn shell_failure_message(exit_code: i32, stderr: &[u8], stdout: &[u8]) -> String {
    const MAX: usize = 8000;
    let mut parts = vec![format!("exit code {exit_code}")];
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    if !stderr.is_empty() {
        parts.push(format!("stderr:\n{}", tail_chars(&stderr, MAX / 2)));
    }
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    if !stdout.is_empty() {
        parts.push(format!("stdout:\n{}", tail_chars(&stdout, MAX / 2)));
    }
    parts.join("\n")
}

fn tail_chars(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let tail = &s[s.len() - max..];
    format!("…{tail}")
}

#[allow(dead_code)]
type _Vault = VaultSession;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::LocalExecutor;
    use crate::id::{MachineId, NodeId, Tag};
    use crate::infra::{
        barrier_node, connect_node, connect_node_id, end_node_on, local_machine, remote_machine,
        shell_node, start_node_id, start_node_on,
    };
    use crate::interactor::NoPromptInteractor;
    use crate::native_exec::{empty_native_executor, LocalNativeExecutor};
    use crate::node::{
        Node, NodeBody, NodeBuilder, OutputChangeRule, OutputMatchStream, PostRunPolicy, Targets,
    };
    use crate::retry::Backoff;
    use infrazeug_native::{
        builtin_registry, EchoInput, MethodRegistry, NativeError, NativeResult, NodeCtx,
        NodeMethod, NATIVE_ECHO, NATIVE_PING,
    };
    use infrazeug_secrets::{FsBackend, PassphraseProvider, VaultRef, VaultStore};
    use infrazeug_shell::{argv, FileSource, PasswordHashSpec, RandomPasswordSpec, ShellOp};
    use infrazeug_shell::{local::ExecOutput, ShellError};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;
    use tokio::sync::oneshot;
    use uuid::Uuid;

    struct ReconnectProbeExecutor {
        reconnect_failures: AtomicUsize,
        health_failures: AtomicUsize,
        reconnect_calls: AtomicUsize,
    }

    impl ReconnectProbeExecutor {
        fn new(reconnect_failures: usize, health_failures: usize) -> Self {
            Self {
                reconnect_failures: AtomicUsize::new(reconnect_failures),
                health_failures: AtomicUsize::new(health_failures),
                reconnect_calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl OpExecutor for ReconnectProbeExecutor {
        async fn execute(
            &self,
            _machine_id: MachineId,
            op: &ShellOp,
        ) -> infrazeug_shell::Result<ExecOutput> {
            let is_health_check =
                matches!(op, ShellOp::Run { argv, .. } if argv == &vec!["true".to_string()]);
            if is_health_check && self.health_failures.load(Ordering::SeqCst) > 0 {
                self.health_failures.fetch_sub(1, Ordering::SeqCst);
                return Err(ShellError::Other("health check failed".into()));
            }
            Ok(ExecOutput {
                exit_code: 0,
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }

        async fn reconnect(&self, _machine_id: MachineId) -> infrazeug_shell::Result<()> {
            self.reconnect_calls.fetch_add(1, Ordering::SeqCst);
            if self.reconnect_failures.load(Ordering::SeqCst) > 0 {
                self.reconnect_failures.fetch_sub(1, Ordering::SeqCst);
                return Err(ShellError::Other("reconnect failed".into()));
            }
            Ok(())
        }
    }

    /// Executor that models a reboot: `cat /proc/.../boot_id` returns successive
    /// queued ids (sticky on the last), `readiness` fails a set number of times
    /// then passes, and `reconnect` fails a set number of times then succeeds.
    struct RebootProbeExecutor {
        boot_ids: std::sync::Mutex<std::collections::VecDeque<String>>,
        boot_probe_calls: AtomicUsize,
        readiness_failures: AtomicUsize,
        readiness_calls: AtomicUsize,
        reconnect_failures: AtomicUsize,
        reconnect_calls: AtomicUsize,
    }

    impl RebootProbeExecutor {
        fn new(boot_ids: Vec<&str>, readiness_failures: usize, reconnect_failures: usize) -> Self {
            Self {
                boot_ids: std::sync::Mutex::new(boot_ids.into_iter().map(String::from).collect()),
                boot_probe_calls: AtomicUsize::new(0),
                readiness_failures: AtomicUsize::new(readiness_failures),
                readiness_calls: AtomicUsize::new(0),
                reconnect_failures: AtomicUsize::new(reconnect_failures),
                reconnect_calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl OpExecutor for RebootProbeExecutor {
        async fn execute(
            &self,
            _machine_id: MachineId,
            op: &ShellOp,
        ) -> infrazeug_shell::Result<ExecOutput> {
            let argv = match op {
                ShellOp::Run { argv, .. } => argv.clone(),
                _ => Vec::new(),
            };
            match argv.first().map(String::as_str) {
                Some("cat") => {
                    self.boot_probe_calls.fetch_add(1, Ordering::SeqCst);
                    let mut q = self.boot_ids.lock().unwrap();
                    let id = if q.len() > 1 {
                        q.pop_front().unwrap()
                    } else {
                        q.front().cloned().unwrap_or_default()
                    };
                    Ok(ExecOutput {
                        exit_code: 0,
                        stdout: id.into_bytes(),
                        stderr: Vec::new(),
                    })
                }
                Some("readiness") => {
                    self.readiness_calls.fetch_add(1, Ordering::SeqCst);
                    if self.readiness_failures.load(Ordering::SeqCst) > 0 {
                        self.readiness_failures.fetch_sub(1, Ordering::SeqCst);
                        return Ok(ExecOutput {
                            exit_code: 1,
                            stdout: Vec::new(),
                            stderr: b"not ready".to_vec(),
                        });
                    }
                    Ok(ExecOutput {
                        exit_code: 0,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                    })
                }
                _ => Ok(ExecOutput {
                    exit_code: 0,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                }),
            }
        }

        async fn reconnect(&self, _machine_id: MachineId) -> infrazeug_shell::Result<()> {
            self.reconnect_calls.fetch_add(1, Ordering::SeqCst);
            if self.reconnect_failures.load(Ordering::SeqCst) > 0 {
                self.reconnect_failures.fetch_sub(1, Ordering::SeqCst);
                return Err(ShellError::Other("reconnect failed".into()));
            }
            Ok(())
        }
    }

    struct BlockingGraphSyncExecutor {
        reboot_node: NodeId,
        sync_seen: Mutex<Option<oneshot::Sender<Vec<(NodeId, NodeStatus)>>>>,
        sync_release: Mutex<Option<oneshot::Receiver<()>>>,
    }

    impl BlockingGraphSyncExecutor {
        fn new(
            reboot_node: NodeId,
            sync_seen: oneshot::Sender<Vec<(NodeId, NodeStatus)>>,
            sync_release: oneshot::Receiver<()>,
        ) -> Self {
            Self {
                reboot_node,
                sync_seen: Mutex::new(Some(sync_seen)),
                sync_release: Mutex::new(Some(sync_release)),
            }
        }
    }

    struct NonZeroExpectedShutdownExecutor {
        reconnect_calls: AtomicUsize,
    }

    #[async_trait]
    impl OpExecutor for NonZeroExpectedShutdownExecutor {
        async fn execute(
            &self,
            _machine_id: MachineId,
            op: &ShellOp,
        ) -> infrazeug_shell::Result<ExecOutput> {
            match op {
                ShellOp::Run { argv, .. } if argv == &vec!["reboot-exit-255".to_string()] => {
                    Ok(ExecOutput {
                        exit_code: 255,
                        stdout: Vec::new(),
                        stderr: b"connection closed by reboot".to_vec(),
                    })
                }
                _ => Ok(ExecOutput {
                    exit_code: 0,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                }),
            }
        }

        async fn reconnect(&self, _machine_id: MachineId) -> infrazeug_shell::Result<()> {
            self.reconnect_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[async_trait]
    impl OpExecutor for BlockingGraphSyncExecutor {
        async fn execute(
            &self,
            _machine_id: MachineId,
            op: &ShellOp,
        ) -> infrazeug_shell::Result<ExecOutput> {
            match op {
                ShellOp::Run { argv, .. } if argv == &vec!["reboot".to_string()] => {
                    Err(ShellError::Other("connection closed by reboot".into()))
                }
                _ => Ok(ExecOutput {
                    exit_code: 0,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                }),
            }
        }

        async fn reconnect(&self, _machine_id: MachineId) -> infrazeug_shell::Result<()> {
            Ok(())
        }

        async fn sync_node_graph_state(
            &self,
            _machine_id: MachineId,
            completed: &[(NodeId, NodeStatus)],
        ) -> infrazeug_shell::Result<()> {
            assert!(
                completed
                    .iter()
                    .any(|(node, status)| *node == self.reboot_node
                        && *status == NodeStatus::Changed),
                "rebooting node must be included before dispose/lock release"
            );
            if let Some(tx) = self.sync_seen.lock().await.take() {
                let _ = tx.send(completed.to_vec());
            }
            if let Some(rx) = self.sync_release.lock().await.take() {
                let _ = rx.await;
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct CountingExecutor {
        calls: std::sync::Mutex<Vec<(MachineId, String)>>,
    }

    impl CountingExecutor {
        fn count(&self, label: &str) -> usize {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, seen)| seen == label)
                .count()
        }

        fn machines_for(&self, label: &str) -> Vec<MachineId> {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .filter_map(|(machine, seen)| (seen == label).then_some(*machine))
                .collect()
        }

        fn labels(&self) -> Vec<String> {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .map(|(_, label)| label.clone())
                .collect()
        }
    }

    #[async_trait]
    impl OpExecutor for CountingExecutor {
        async fn execute(
            &self,
            machine_id: MachineId,
            op: &ShellOp,
        ) -> infrazeug_shell::Result<ExecOutput> {
            let label = match op {
                ShellOp::Run { argv, .. } => argv.first().cloned().unwrap_or_default(),
                _ => "op".into(),
            };
            self.calls.lock().unwrap().push((machine_id, label.clone()));
            Ok(ExecOutput {
                exit_code: 0,
                stdout: label.into_bytes(),
                stderr: Vec::new(),
            })
        }
    }

    fn counted_node(
        id: NodeId,
        name: &str,
        targets: Targets,
        run_policy: RunPolicy,
        deps: Vec<NodeId>,
    ) -> Node {
        let mut node = shell_node(id, name, ShellOp::run(vec![name.to_string()]), targets);
        node.policy.run_policy = run_policy;
        node.deps = deps;
        node
    }

    async fn run_with_native(
        infra: &Infra,
        methods: infrazeug_native::MethodRegistry,
    ) -> RunReport {
        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let executor: Arc<dyn OpExecutor> = Arc::new(LocalExecutor);
        let native_executor: Arc<dyn NativeExecutor> =
            Arc::new(LocalNativeExecutor::new(Arc::new(methods)));

        DefaultScheduler
            .run(SchedRuntime {
                infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::default()),
                executor,
                native_executor,
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap()
    }

    async fn run_counted(infra: &Infra, executor: Arc<CountingExecutor>) -> RunReport {
        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let executor: Arc<dyn OpExecutor> = executor;

        DefaultScheduler
            .run(SchedRuntime {
                infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::default()),
                executor,
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn tagged_filter_apply_runs_connect_and_selected_nodes() {
        let web_machine = MachineId(Uuid::new_v4());
        let db_machine = MachineId(Uuid::new_v4());
        let web_connect = connect_node_id(web_machine);
        let db_connect = connect_node_id(db_machine);
        let web_base = NodeId(Uuid::new_v4());
        let web = NodeId(Uuid::new_v4());
        let db = NodeId(Uuid::new_v4());

        let mut web_base_node = shell_node(
            web_base,
            "base/web",
            ShellOp::run(vec!["base/web".into()]),
            Targets::Machine(web_machine),
        );
        web_base_node.deps.push(web_connect);
        let mut web_node = shell_node(
            web,
            "web",
            ShellOp::run(vec!["web".into()]),
            Targets::Machine(web_machine),
        );
        web_node.deps.push(web_base);
        web_node.tags.push(Tag::new("app", "web"));
        let mut db_node = shell_node(
            db,
            "db",
            ShellOp::run(vec!["db".into()]),
            Targets::Machine(db_machine),
        );
        db_node.deps.push(db_connect);

        let infra = Infra::new()
            .add_machine(local_machine(web_machine, "web-host"))
            .unwrap()
            .add_machine(local_machine(db_machine, "db-host"))
            .unwrap()
            .add_node(start_node_on(web_machine))
            .unwrap()
            .add_node(connect_node(
                web_connect,
                "connect/web-host",
                Targets::Machine(web_machine),
                vec![start_node_id()],
            ))
            .unwrap()
            .add_node(connect_node(
                db_connect,
                "connect/db-host",
                Targets::Machine(db_machine),
                vec![start_node_id()],
            ))
            .unwrap()
            .add_node(web_base_node)
            .unwrap()
            .add_node(web_node)
            .unwrap()
            .add_node(db_node)
            .unwrap()
            .add_node(end_node_on(web_machine, vec![web, db]))
            .unwrap()
            .with_tag_filter(&["web".to_string()]);

        let executor = Arc::new(CountingExecutor::default());
        let report = tokio::time::timeout(
            Duration::from_secs(2),
            run_counted(&infra, Arc::clone(&executor)),
        )
        .await
        .expect("filtered apply should make progress");

        let labels = executor.labels();
        assert!(labels.contains(&"true".to_string()));
        assert!(labels.contains(&"base/web".to_string()));
        assert!(labels.contains(&"web".to_string()));
        assert!(!labels.contains(&"db".to_string()));

        let names: std::collections::HashSet<&str> = report
            .entries
            .iter()
            .map(|e| e.node_name.as_str())
            .collect();
        assert!(names.contains("start"));
        assert!(names.contains("connect/web-host"));
        assert!(names.contains("base/web"));
        assert!(names.contains("web"));
        assert!(names.contains("end"));
        assert!(!names.contains("connect/db-host"));
        assert!(!names.contains("db"));
    }

    struct CloudCliExecutor;

    #[async_trait]
    impl OpExecutor for CloudCliExecutor {
        async fn execute(
            &self,
            _machine_id: MachineId,
            op: &ShellOp,
        ) -> infrazeug_shell::Result<ExecOutput> {
            match op {
                ShellOp::Run { argv, .. } if argv == &argv!["cloudctl", "create-key"] => {
                    Ok(ExecOutput {
                        exit_code: 0,
                        stdout: b"created bucket key: ak_live_123\n".to_vec(),
                        stderr: Vec::new(),
                    })
                }
                _ => Err(ShellError::Other("unexpected op".into())),
            }
        }
    }

    #[tokio::test]
    async fn vault_write_stores_transformed_capture() {
        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod-runtime", "hunter2", "recovery")
            .await
            .unwrap();

        let machine = MachineId(Uuid::new_v4());
        let create = NodeId(Uuid::new_v4());
        let save = NodeId(Uuid::new_v4());
        let mut save_node = shell_node(
            save,
            "store bucket key",
            ShellOp::mutable_vault_write(
                "prod-runtime",
                "cloud/images.vault",
                "credentials.access_key",
                FileSource::capture_same_machine(create.0)
                    .regex_include("key: ([A-Za-z0-9_]+)")
                    .trim(),
            ),
            Targets::Machine(machine),
        );
        save_node.deps = vec![create];

        let infra = Infra::new()
            .add_machine(local_machine(machine, "local"))
            .unwrap()
            .add_node(shell_node(
                create,
                "create bucket key",
                ShellOp::run(argv!["cloudctl", "create-key"]),
                Targets::Machine(machine),
            ))
            .unwrap()
            .add_node(save_node)
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::from_store(store, Vec::new())),
                executor: Arc::new(CloudCliExecutor),
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();
        assert!(
            report
                .entries
                .iter()
                .any(|entry| entry.node_id == save && entry.status == NodeStatus::Changed),
            "vault write node should report changed"
        );

        let mut verify = VaultStore::new(
            Arc::new(FsBackend::new(dir.path())),
            dir.path().to_path_buf(),
        );
        verify
            .unlock_with_provider(
                "prod-runtime",
                &PassphraseProvider::new("hunter2"),
                "recovery",
            )
            .await
            .unwrap();
        let saved = verify
            .resolve_field(&VaultRef::mutable_field(
                "cloud/images.vault",
                "credentials.access_key",
            ))
            .await
            .unwrap();
        assert_eq!(saved, Value::Text("ak_live_123".into()));
    }

    #[tokio::test]
    async fn vault_ensure_password_hash_generates_once() {
        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod-runtime", "hunter2", "recovery")
            .await
            .unwrap();

        let machine = MachineId(Uuid::new_v4());
        let ensure = NodeId(Uuid::new_v4());
        let infra = Infra::new()
            .add_machine(local_machine(machine, "local"))
            .unwrap()
            .add_node(shell_node(
                ensure,
                "ensure password hash",
                ShellOp::mutable_vault_ensure_random_password_hash(
                    "prod-runtime",
                    "apps/demo.vault",
                    "password",
                    "password_hash",
                    RandomPasswordSpec::new(12).special("!"),
                    PasswordHashSpec::argon2id().m_cost(8).t_cost(1),
                ),
                Targets::Machine(machine),
            ))
            .unwrap();

        let run = |store| async {
            let plan = infra.plan().unwrap();
            let (events, _) = tokio::sync::broadcast::channel(32);
            let (_tx, cmd_rx) = mpsc::channel(4);
            DefaultScheduler
                .run(SchedRuntime {
                    infra: &infra,
                    plan,
                    limits: GlobalLimits::default(),
                    events,
                    commands: cmd_rx,
                    interact: Arc::new(NoPromptInteractor),
                    cancel: CancellationToken::new(),
                    vault: Arc::new(VaultSession::from_store(store, Vec::new())),
                    executor: Arc::new(LocalExecutor),
                    native_executor: empty_native_executor(),
                    hash_relay: None,
                    captures: Arc::new(CaptureStore::new()),
                    capture_spill_root: None,
                })
                .await
                .unwrap()
        };

        let report = run(store).await;
        assert_eq!(
            report
                .entries
                .iter()
                .find(|entry| entry.node_id == ensure)
                .unwrap()
                .status,
            NodeStatus::Changed
        );

        let mut verify = VaultStore::new(
            Arc::new(FsBackend::new(dir.path())),
            dir.path().to_path_buf(),
        );
        verify
            .unlock_with_provider(
                "prod-runtime",
                &PassphraseProvider::new("hunter2"),
                "recovery",
            )
            .await
            .unwrap();
        let password = verify
            .resolve_field(&VaultRef::mutable_field("apps/demo.vault", "password"))
            .await
            .unwrap();
        let hash = verify
            .resolve_field(&VaultRef::mutable_field("apps/demo.vault", "password_hash"))
            .await
            .unwrap();
        let Value::Text(hash_text) = &hash else {
            panic!("expected text hash");
        };
        assert!(hash_text.starts_with("$argon2id$v=19$m=8,t=1,p=1$"));

        let report = run(verify).await;
        assert_eq!(
            report
                .entries
                .iter()
                .find(|entry| entry.node_id == ensure)
                .unwrap()
                .status,
            NodeStatus::Unchanged
        );

        let mut verify = VaultStore::new(
            Arc::new(FsBackend::new(dir.path())),
            dir.path().to_path_buf(),
        );
        verify
            .unlock_with_provider(
                "prod-runtime",
                &PassphraseProvider::new("hunter2"),
                "recovery",
            )
            .await
            .unwrap();
        assert_eq!(
            verify
                .resolve_field(&VaultRef::mutable_field("apps/demo.vault", "password"))
                .await
                .unwrap(),
            password
        );
        assert_eq!(
            verify
                .resolve_field(&VaultRef::mutable_field("apps/demo.vault", "password_hash"))
                .await
                .unwrap(),
            hash
        );
    }

    /// A `Seq` composed purely of vault writes must run controller-side (not be
    /// dispatched to a shell executor), and the random write-if-absent variants
    /// must keep their first-run values stable across applies.
    #[tokio::test]
    async fn seq_of_random_vault_writes_runs_controller_side_write_if_absent() {
        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod-runtime", "hunter2", "recovery")
            .await
            .unwrap();

        let machine = MachineId(Uuid::new_v4());
        let secrets = NodeId(Uuid::new_v4());
        let infra = Infra::new()
            .add_machine(local_machine(machine, "local"))
            .unwrap()
            .add_node(shell_node(
                secrets,
                "generate app secrets",
                ShellOp::Seq {
                    steps: vec![
                        ShellOp::mutable_vault_write_random_password(
                            "prod-runtime",
                            "apps/demo.vault",
                            "salt",
                            RandomPasswordSpec::new(16),
                        ),
                        ShellOp::mutable_vault_write_random_password(
                            "prod-runtime",
                            "apps/demo.vault",
                            "encryption_key",
                            RandomPasswordSpec::new(16),
                        ),
                    ],
                },
                Targets::Machine(machine),
            ))
            .unwrap();

        let run = |store| async {
            let plan = infra.plan().unwrap();
            let (events, _) = tokio::sync::broadcast::channel(32);
            let (_tx, cmd_rx) = mpsc::channel(4);
            DefaultScheduler
                .run(SchedRuntime {
                    infra: &infra,
                    plan,
                    limits: GlobalLimits::default(),
                    events,
                    commands: cmd_rx,
                    interact: Arc::new(NoPromptInteractor),
                    cancel: CancellationToken::new(),
                    vault: Arc::new(VaultSession::from_store(store, Vec::new())),
                    executor: Arc::new(LocalExecutor),
                    native_executor: empty_native_executor(),
                    hash_relay: None,
                    captures: Arc::new(CaptureStore::new()),
                    capture_spill_root: None,
                })
                .await
                .unwrap()
        };

        let report = run(store).await;
        assert_eq!(
            report
                .entries
                .iter()
                .find(|entry| entry.node_id == secrets)
                .unwrap()
                .status,
            NodeStatus::Changed
        );

        let mut verify = VaultStore::new(
            Arc::new(FsBackend::new(dir.path())),
            dir.path().to_path_buf(),
        );
        verify
            .unlock_with_provider(
                "prod-runtime",
                &PassphraseProvider::new("hunter2"),
                "recovery",
            )
            .await
            .unwrap();
        let salt = verify
            .resolve_field(&VaultRef::mutable_field("apps/demo.vault", "salt"))
            .await
            .unwrap();
        let encryption_key = verify
            .resolve_field(&VaultRef::mutable_field(
                "apps/demo.vault",
                "encryption_key",
            ))
            .await
            .unwrap();

        let report = run(verify).await;
        assert_eq!(
            report
                .entries
                .iter()
                .find(|entry| entry.node_id == secrets)
                .unwrap()
                .status,
            NodeStatus::Unchanged
        );

        let mut verify = VaultStore::new(
            Arc::new(FsBackend::new(dir.path())),
            dir.path().to_path_buf(),
        );
        verify
            .unlock_with_provider(
                "prod-runtime",
                &PassphraseProvider::new("hunter2"),
                "recovery",
            )
            .await
            .unwrap();
        assert_eq!(
            verify
                .resolve_field(&VaultRef::mutable_field("apps/demo.vault", "salt"))
                .await
                .unwrap(),
            salt
        );
        assert_eq!(
            verify
                .resolve_field(&VaultRef::mutable_field(
                    "apps/demo.vault",
                    "encryption_key"
                ))
                .await
                .unwrap(),
            encryption_key
        );
    }

    /// Native method that reads an API credential from the controller vault and
    /// emits it as a JSON capture — the same shape the OVH ensure nodes use.
    struct ReadCredEmit;

    #[async_trait]
    impl NodeMethod for ReadCredEmit {
        type Input = ();
        type Output = serde_json::Value;

        fn name(&self) -> &'static str {
            "test.read_cred_emit"
        }

        async fn execute(
            &self,
            ctx: &NodeCtx,
            _input: (),
        ) -> infrazeug_native::Result<NativeResult> {
            let source = ctx
                .secrets
                .as_ref()
                .ok_or_else(|| NativeError::other("no secret source on ctx"))?;
            let bytes = source
                .read_field("cloud/ovh.vault", "application_key")
                .await?;
            let access_key =
                String::from_utf8(bytes).map_err(|e| NativeError::other(e.to_string()))?;
            let capture = serde_json::json!({ "access_key_id": access_key });
            NativeResult::changed("read credential")
                .with_json_capture(&capture)
                .map_err(|e| NativeError::other(e.to_string()))
        }
    }

    #[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
    struct ReadCaptureInput {
        node: Uuid,
    }

    struct ReadCapture;

    #[async_trait]
    impl NodeMethod for ReadCapture {
        type Input = ReadCaptureInput;
        type Output = ();

        fn name(&self) -> &'static str {
            "test.read_capture"
        }

        async fn execute(
            &self,
            ctx: &NodeCtx,
            input: Self::Input,
        ) -> infrazeug_native::Result<NativeResult> {
            let source = ctx
                .secrets
                .as_ref()
                .ok_or_else(|| NativeError::other("no input source on ctx"))?;
            let bytes = source.read_node_capture(input.node, ctx.machine_id).await?;
            let text = String::from_utf8(bytes).map_err(|e| NativeError::other(e.to_string()))?;
            Ok(NativeResult::changed(format!("capture={text}")))
        }
    }

    #[tokio::test]
    async fn native_method_reads_upstream_node_capture() {
        let machine = MachineId(Uuid::new_v4());
        let seed = NodeId(Uuid::new_v4());
        let reader = NodeId(Uuid::new_v4());

        let mut read_node = NodeBuilder::native_with_input(
            reader,
            "test.read_capture",
            infrazeug_native::encode_input(&ReadCaptureInput { node: seed.0 }).unwrap(),
            Targets::Machine(machine),
        )
        .name("read capture")
        .build();
        read_node.deps = vec![seed];

        let infra = Infra::new()
            .add_machine(local_machine(machine, "local"))
            .unwrap()
            .add_node(shell_node(
                seed,
                "seed",
                ShellOp::run(vec!["printf".to_string(), "seed".to_string()]),
                Targets::Machine(machine),
            ))
            .unwrap()
            .add_node(read_node)
            .unwrap();

        let mut registry = MethodRegistry::new();
        registry.register(ReadCapture);

        let report = run_with_native(&infra, registry).await;
        let entry = report
            .entries
            .iter()
            .find(|entry| entry.node_id == reader)
            .unwrap();
        assert_eq!(entry.status, NodeStatus::Changed);
        assert_eq!(entry.message.as_deref(), Some("capture=seed"));
    }

    /// End-to-end: one run that *reads* an API credential from the vault (new
    /// SecretSource path) and *writes* a derived secret back to the mutable vault
    /// (existing VaultWrite path), sharing a single unlocked `VaultSession`.
    #[tokio::test]
    async fn native_reads_cred_then_writes_mutable_vault() {
        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod-runtime", "hunter2", "recovery")
            .await
            .unwrap();
        // Seed the API credential that the native node will read.
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "application_key".to_string(),
            Value::Text("AKID-live".into()),
        );
        store
            .put_vault_fields("prod-runtime", "cloud/ovh.vault", &fields)
            .await
            .unwrap();

        let machine = MachineId(Uuid::new_v4());
        let reader = NodeId(Uuid::new_v4());
        let save = NodeId(Uuid::new_v4());

        let mut save_node = shell_node(
            save,
            "store derived key",
            ShellOp::mutable_vault_write(
                "prod-runtime",
                "cloud/out.vault",
                "credentials.access_key",
                FileSource::capture_same_machine(reader.0).json_pointer("/access_key_id"),
            ),
            Targets::Machine(machine),
        );
        save_node.deps = vec![reader];

        let infra = Infra::new()
            .add_machine(local_machine(machine, "local"))
            .unwrap()
            .add_node(
                NodeBuilder::native_with_input(
                    reader,
                    "test.read_cred_emit",
                    serde_cbor::Value::Null,
                    Targets::Machine(machine),
                )
                .name("read cred")
                .build(),
            )
            .unwrap()
            .add_node(save_node)
            .unwrap();

        let mut registry = MethodRegistry::new();
        registry.register(ReadCredEmit);

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::from_store(store, Vec::new())),
                executor: Arc::new(CloudCliExecutor),
                native_executor: Arc::new(LocalNativeExecutor::new(Arc::new(registry))),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();

        // Both nodes succeeded.
        for node in [reader, save] {
            assert_eq!(
                report
                    .entries
                    .iter()
                    .find(|e| e.node_id == node)
                    .unwrap()
                    .status,
                NodeStatus::Changed,
            );
        }

        // The credential read from the vault was written back into the mutable vault.
        let mut verify = VaultStore::new(
            Arc::new(FsBackend::new(dir.path())),
            dir.path().to_path_buf(),
        );
        verify
            .unlock_with_provider(
                "prod-runtime",
                &PassphraseProvider::new("hunter2"),
                "recovery",
            )
            .await
            .unwrap();
        let saved = verify
            .resolve_field(&VaultRef::mutable_field(
                "cloud/out.vault",
                "credentials.access_key",
            ))
            .await
            .unwrap();
        assert_eq!(saved, Value::Text("AKID-live".into()));
    }

    #[tokio::test]
    async fn barrier_preserves_upstream_change_without_shell_execution() {
        let m = MachineId(Uuid::new_v4());
        let seed = NodeId(Uuid::new_v4());
        let barrier = NodeId(Uuid::new_v4());
        let consumer = NodeId(Uuid::new_v4());

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(counted_node(
                seed,
                "seed",
                Targets::Machine(m),
                RunPolicy::Always,
                vec![],
            ))
            .unwrap()
            .add_node(barrier_node(
                barrier,
                "barrier",
                Targets::Machine(m),
                vec![seed],
            ))
            .unwrap()
            .add_node(counted_node(
                consumer,
                "consumer",
                Targets::Machine(m),
                RunPolicy::OnUpstreamChange,
                vec![barrier],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;
        let entry = |node_id| {
            report
                .entries
                .iter()
                .find(|entry| entry.node_id == node_id)
                .unwrap()
        };

        assert_eq!(executor.count("seed"), 1);
        assert_eq!(executor.count("barrier"), 0);
        assert_eq!(executor.count("consumer"), 1);
        assert_eq!(entry(seed).status, NodeStatus::Changed);
        assert_eq!(entry(barrier).status, NodeStatus::Changed);
        assert_eq!(entry(consumer).status, NodeStatus::Changed);
    }

    #[tokio::test]
    async fn barrier_does_not_create_false_change() {
        let m = MachineId(Uuid::new_v4());
        let seed = NodeId(Uuid::new_v4());
        let barrier = NodeId(Uuid::new_v4());
        let consumer = NodeId(Uuid::new_v4());

        let mut seed_node =
            counted_node(seed, "seed", Targets::Machine(m), RunPolicy::Always, vec![]);
        seed_node.policy.success.change_policy.rules.push(
            OutputChangeRule::unchanged_when_contains(OutputMatchStream::Stdout, "seed"),
        );

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(seed_node)
            .unwrap()
            .add_node(barrier_node(
                barrier,
                "barrier",
                Targets::Machine(m),
                vec![seed],
            ))
            .unwrap()
            .add_node(counted_node(
                consumer,
                "consumer",
                Targets::Machine(m),
                RunPolicy::OnUpstreamChange,
                vec![barrier],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;
        let entry = |node_id| {
            report
                .entries
                .iter()
                .find(|entry| entry.node_id == node_id)
                .unwrap()
        };

        assert_eq!(executor.count("seed"), 1);
        assert_eq!(executor.count("barrier"), 0);
        assert_eq!(executor.count("consumer"), 0);
        assert_eq!(entry(seed).status, NodeStatus::Unchanged);
        assert_eq!(entry(barrier).status, NodeStatus::Unchanged);
        assert_eq!(entry(consumer).status, NodeStatus::Skipped);
    }

    #[tokio::test]
    async fn native_echo_propagates_to_downstream() {
        let m = MachineId(Uuid::new_v4());
        let echo = NodeId(Uuid::new_v4());
        let after = NodeId(Uuid::new_v4());
        let input = serde_cbor::Value::Bytes(
            serde_cbor::to_vec(&EchoInput {
                text: "propagate".into(),
            })
            .unwrap(),
        );

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(
                NodeBuilder::native_with_input(echo, NATIVE_ECHO, input, Targets::Machine(m))
                    .name("echo")
                    .build(),
            )
            .unwrap()
            .add_node(counted_node(
                after,
                "after",
                Targets::Machine(m),
                RunPolicy::OnUpstreamChange,
                vec![echo],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::default()),
                executor: executor.clone(),
                native_executor: Arc::new(LocalNativeExecutor::new(Arc::new(builtin_registry()))),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();
        assert_eq!(executor.count("after"), 1);
        assert_eq!(
            report
                .entries
                .iter()
                .find(|e| e.node_id == echo)
                .unwrap()
                .status,
            NodeStatus::Changed
        );
        assert_eq!(
            report
                .entries
                .iter()
                .find(|e| e.node_id == after)
                .unwrap()
                .status,
            NodeStatus::Changed
        );
    }

    #[tokio::test]
    async fn native_ping_skips_downstream() {
        let m = MachineId(Uuid::new_v4());
        let ping = NodeId(Uuid::new_v4());
        let after = NodeId(Uuid::new_v4());

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(
                NodeBuilder::native_with_input(
                    ping,
                    NATIVE_PING,
                    serde_cbor::Value::Null,
                    Targets::Machine(m),
                )
                .name("ping")
                .build(),
            )
            .unwrap()
            .add_node(counted_node(
                after,
                "after",
                Targets::Machine(m),
                RunPolicy::OnUpstreamChange,
                vec![ping],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::default()),
                executor: executor.clone(),
                native_executor: Arc::new(LocalNativeExecutor::new(Arc::new(builtin_registry()))),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();

        assert_eq!(executor.count("after"), 0);
        assert_eq!(
            report
                .entries
                .iter()
                .find(|e| e.node_id == ping)
                .unwrap()
                .status,
            NodeStatus::Unchanged
        );
        assert_eq!(
            report
                .entries
                .iter()
                .find(|e| e.node_id == after)
                .unwrap()
                .status,
            NodeStatus::Skipped
        );
    }

    #[test]
    fn native_on_agentless_fails_lint() {
        let m = MachineId(Uuid::new_v4());
        let infra = Infra::new()
            .with_default_remote_transport(crate::transport::TransportChoice::SshAgentless)
            .add_machine(remote_machine(
                m,
                "remote",
                crate::machine::SshConfig::new("root@example"),
            ))
            .unwrap()
            .add_node(Node {
                id: NodeId(Uuid::new_v4()),
                name: "native".into(),
                description: None,
                body: NodeBody::Native {
                    method_id: "test".into(),
                    input: serde_cbor::Value::Null,
                },
                targets: Targets::Machine(m),
                deps: Vec::new(),
                tags: Vec::new(),
                policy: Default::default(),
            })
            .unwrap();
        assert!(infra.lint().is_err());
    }

    #[tokio::test]
    async fn lazy_runs_when_consumer_runs() {
        let check_machine = MachineId(Uuid::new_v4());
        let build_machine_a = MachineId(Uuid::new_v4());
        let build_machine_b = MachineId(Uuid::new_v4());
        let check = NodeId(Uuid::new_v4());
        let build = NodeId(Uuid::new_v4());
        let reboot = NodeId(Uuid::new_v4());

        let infra = Infra::new()
            .add_machine(local_machine(check_machine, "check-machine"))
            .unwrap()
            .add_machine(local_machine(build_machine_a, "build-machine-a"))
            .unwrap()
            .add_machine(local_machine(build_machine_b, "build-machine-b"))
            .unwrap()
            .add_node(counted_node(
                check,
                "check",
                Targets::Machine(check_machine),
                RunPolicy::Always,
                Vec::new(),
            ))
            .unwrap()
            .add_node(counted_node(
                build,
                "build",
                Targets::Machines(vec![build_machine_a, build_machine_b]),
                RunPolicy::Lazy,
                vec![check],
            ))
            .unwrap()
            .add_node(counted_node(
                reboot,
                "reboot",
                Targets::Machine(check_machine),
                RunPolicy::OnUpstreamChange,
                vec![check, build],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;

        let build_machines = executor.machines_for("build");
        assert_eq!(build_machines.len(), 2);
        assert!(build_machines.contains(&build_machine_a));
        assert!(build_machines.contains(&build_machine_b));
        assert_eq!(executor.count("reboot"), 1);
        assert_eq!(
            report
                .entries
                .iter()
                .filter(|entry| entry.node_id == build && entry.status == NodeStatus::Changed)
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn lazy_skipped_when_consumer_skipped() {
        let m = MachineId(Uuid::new_v4());
        let check = NodeId(Uuid::new_v4());
        let build = NodeId(Uuid::new_v4());
        let reboot = NodeId(Uuid::new_v4());

        let mut check_node = counted_node(
            check,
            "check",
            Targets::Machine(m),
            RunPolicy::Always,
            Vec::new(),
        );
        check_node.policy.success.change_policy.rules.push(
            OutputChangeRule::unchanged_when_contains(OutputMatchStream::Stdout, "check"),
        );

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(check_node)
            .unwrap()
            .add_node(counted_node(
                build,
                "build",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![check],
            ))
            .unwrap()
            .add_node(counted_node(
                reboot,
                "reboot",
                Targets::Machine(m),
                RunPolicy::OnUpstreamChange,
                vec![check, build],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;
        let entry = |node_id| {
            report
                .entries
                .iter()
                .find(|entry| entry.node_id == node_id)
                .unwrap()
        };

        assert_eq!(executor.count("build"), 0);
        assert_eq!(entry(build).status, NodeStatus::Skipped);
        assert_eq!(entry(build).message.as_deref(), Some("unchanged"));
        assert_eq!(entry(reboot).status, NodeStatus::Skipped);
    }

    #[tokio::test]
    async fn lazy_no_dependents_skipped() {
        let m = MachineId(Uuid::new_v4());
        let build = NodeId(Uuid::new_v4());
        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(counted_node(
                build,
                "build",
                Targets::Machine(m),
                RunPolicy::Lazy,
                Vec::new(),
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;

        assert_eq!(executor.count("build"), 0);
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].node_id, build);
        assert_eq!(report.entries[0].status, NodeStatus::Skipped);
        assert_eq!(report.entries[0].message.as_deref(), Some("not demanded"));
    }

    #[tokio::test]
    async fn lazy_consumer_run_intent_survives_lazy_unchanged() {
        let m = MachineId(Uuid::new_v4());
        let build = NodeId(Uuid::new_v4());
        let consumer = NodeId(Uuid::new_v4());

        let mut build_node = counted_node(
            build,
            "build",
            Targets::Machine(m),
            RunPolicy::Lazy,
            Vec::new(),
        );
        build_node.policy.success.change_policy.rules.push(
            OutputChangeRule::unchanged_when_contains(OutputMatchStream::Stdout, "build"),
        );

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(build_node)
            .unwrap()
            .add_node(counted_node(
                consumer,
                "consumer",
                Targets::Machine(m),
                RunPolicy::OnUpstreamChange,
                vec![build],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;
        let entry = |node_id| {
            report
                .entries
                .iter()
                .find(|entry| entry.node_id == node_id)
                .unwrap()
        };

        assert_eq!(executor.count("build"), 1);
        assert_eq!(executor.count("consumer"), 1);
        assert_eq!(entry(build).status, NodeStatus::Unchanged);
        assert_eq!(entry(consumer).status, NodeStatus::Changed);
    }

    #[tokio::test]
    async fn unknown_consumer_demands_lazy_dep_after_strict_deps() {
        let m = MachineId(Uuid::new_v4());
        let gate = NodeId(Uuid::new_v4());
        let build = NodeId(Uuid::new_v4());
        let consumer = NodeId(Uuid::new_v4());

        let mut gate_node = counted_node(
            gate,
            "gate",
            Targets::Machine(m),
            RunPolicy::Always,
            Vec::new(),
        );
        gate_node.policy.success.change_policy.rules.push(
            OutputChangeRule::unchanged_when_contains(OutputMatchStream::Stdout, "gate"),
        );

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(gate_node)
            .unwrap()
            .add_node(counted_node(
                build,
                "build",
                Targets::Machine(m),
                RunPolicy::Lazy,
                Vec::new(),
            ))
            .unwrap()
            .add_node(counted_node(
                consumer,
                "consumer",
                Targets::Machine(m),
                RunPolicy::OnUpstreamChange,
                vec![gate, build],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;
        let entry = |node_id| {
            report
                .entries
                .iter()
                .find(|entry| entry.node_id == node_id)
                .unwrap()
        };

        assert_eq!(executor.labels(), vec!["gate", "build", "consumer"]);
        assert_eq!(entry(gate).status, NodeStatus::Unchanged);
        assert_eq!(entry(build).status, NodeStatus::Changed);
        assert_eq!(entry(consumer).status, NodeStatus::Changed);
    }

    #[tokio::test]
    async fn lazy_dep_guarded_by_unchanged_strict_dep_is_skipped() {
        let m = MachineId(Uuid::new_v4());
        let check = NodeId(Uuid::new_v4());
        let build = NodeId(Uuid::new_v4());
        let install = NodeId(Uuid::new_v4());

        let mut check_node = counted_node(
            check,
            "check",
            Targets::Machine(m),
            RunPolicy::Always,
            Vec::new(),
        );
        check_node.policy.success.change_policy.rules.push(
            OutputChangeRule::unchanged_when_contains(OutputMatchStream::Stdout, "check"),
        );

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(check_node)
            .unwrap()
            .add_node(counted_node(
                build,
                "build",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![check],
            ))
            .unwrap()
            .add_node(counted_node(
                install,
                "install",
                Targets::Machine(m),
                RunPolicy::OnUpstreamChange,
                vec![check, build],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;
        let entry = |node_id| {
            report
                .entries
                .iter()
                .find(|entry| entry.node_id == node_id)
                .unwrap()
        };

        assert_eq!(executor.labels(), vec!["check"]);
        assert_eq!(entry(check).status, NodeStatus::Unchanged);
        assert_eq!(entry(build).status, NodeStatus::Skipped);
        assert_eq!(entry(install).status, NodeStatus::Skipped);
    }

    #[tokio::test]
    async fn lazy_chain_demand_propagates() {
        let m = MachineId(Uuid::new_v4());
        let a = NodeId(Uuid::new_v4());
        let b = NodeId(Uuid::new_v4());
        let c = NodeId(Uuid::new_v4());
        let d = NodeId(Uuid::new_v4());

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(counted_node(
                a,
                "a",
                Targets::Machine(m),
                RunPolicy::Lazy,
                Vec::new(),
            ))
            .unwrap()
            .add_node(counted_node(
                b,
                "b",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![a],
            ))
            .unwrap()
            .add_node(counted_node(
                c,
                "c",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![b],
            ))
            .unwrap()
            .add_node(counted_node(
                d,
                "d",
                Targets::Machine(m),
                RunPolicy::Always,
                vec![c],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;

        assert_eq!(executor.labels(), vec!["a", "b", "c", "d"]);
        assert_eq!(report.entries.len(), 4);
        assert!(report
            .entries
            .iter()
            .all(|entry| entry.status == NodeStatus::Changed));
    }

    #[tokio::test]
    async fn lazy_demanded_once_by_multiple() {
        let m = MachineId(Uuid::new_v4());
        let build = NodeId(Uuid::new_v4());
        let consumer_a = NodeId(Uuid::new_v4());
        let consumer_b = NodeId(Uuid::new_v4());

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(counted_node(
                build,
                "build",
                Targets::Machine(m),
                RunPolicy::Lazy,
                Vec::new(),
            ))
            .unwrap()
            .add_node(counted_node(
                consumer_a,
                "consumer-a",
                Targets::Machine(m),
                RunPolicy::Always,
                vec![build],
            ))
            .unwrap()
            .add_node(counted_node(
                consumer_b,
                "consumer-b",
                Targets::Machine(m),
                RunPolicy::Always,
                vec![build],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;

        assert_eq!(executor.count("build"), 1);
        assert_eq!(executor.count("consumer-a"), 1);
        assert_eq!(executor.count("consumer-b"), 1);
        assert_eq!(report.entries.len(), 3);
    }

    /// A whole lazy chain with no consumer stays dormant: dormancy propagates
    /// back through the chain (each node's only dependent is itself skipped).
    #[tokio::test]
    async fn lazy_chain_fully_dormant_without_consumer() {
        let m = MachineId(Uuid::new_v4());
        let a = NodeId(Uuid::new_v4());
        let b = NodeId(Uuid::new_v4());
        let c = NodeId(Uuid::new_v4());

        // a <- b <- c, all lazy, nothing depends on the leaf `c`.
        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(counted_node(
                a,
                "a",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![],
            ))
            .unwrap()
            .add_node(counted_node(
                b,
                "b",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![a],
            ))
            .unwrap()
            .add_node(counted_node(
                c,
                "c",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![b],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;
        let entry = |id| report.entries.iter().find(|e| e.node_id == id).unwrap();

        assert!(executor.labels().is_empty(), "no lazy node should run");
        for id in [a, b, c] {
            assert_eq!(entry(id).status, NodeStatus::Skipped);
            assert_eq!(entry(id).message.as_deref(), Some("not demanded"));
        }
    }

    /// Two independent lazy chains in one graph: a demanded chain runs end to end
    /// while an undemanded chain stays dormant. Demand does not leak between chains.
    #[tokio::test]
    async fn multiple_lazy_chains_demanded_and_dormant_coexist() {
        let m = MachineId(Uuid::new_v4());
        // Demanded chain X: xa <- xb <- xc <- xd(Always).
        let (xa, xb, xc, xd) = (
            NodeId(Uuid::new_v4()),
            NodeId(Uuid::new_v4()),
            NodeId(Uuid::new_v4()),
            NodeId(Uuid::new_v4()),
        );
        // Dormant chain Y: ya <- yb <- yc, no consumer.
        let (ya, yb, yc) = (
            NodeId(Uuid::new_v4()),
            NodeId(Uuid::new_v4()),
            NodeId(Uuid::new_v4()),
        );

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(counted_node(
                xa,
                "xa",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![],
            ))
            .unwrap()
            .add_node(counted_node(
                xb,
                "xb",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![xa],
            ))
            .unwrap()
            .add_node(counted_node(
                xc,
                "xc",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![xb],
            ))
            .unwrap()
            .add_node(counted_node(
                xd,
                "xd",
                Targets::Machine(m),
                RunPolicy::Always,
                vec![xc],
            ))
            .unwrap()
            .add_node(counted_node(
                ya,
                "ya",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![],
            ))
            .unwrap()
            .add_node(counted_node(
                yb,
                "yb",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![ya],
            ))
            .unwrap()
            .add_node(counted_node(
                yc,
                "yc",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![yb],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;
        let entry = |id| report.entries.iter().find(|e| e.node_id == id).unwrap();

        // Demanded chain ran end to end.
        for (id, name) in [(xa, "xa"), (xb, "xb"), (xc, "xc"), (xd, "xd")] {
            assert_eq!(executor.count(name), 1, "{name} should run once");
            assert_eq!(entry(id).status, NodeStatus::Changed);
        }
        // Dormant chain never ran.
        for (id, name) in [(ya, "ya"), (yb, "yb"), (yc, "yc")] {
            assert_eq!(executor.count(name), 0, "{name} should stay dormant");
            assert_eq!(entry(id).status, NodeStatus::Skipped);
            assert_eq!(entry(id).message.as_deref(), Some("not demanded"));
        }
    }

    /// A lazy node feeding two lazy branches: demand reaches the shared head and the
    /// branch that has a consumer, while the consumer-less branch stays dormant even
    /// though its (lazy) dependency ran.
    #[tokio::test]
    async fn lazy_branch_demand_reaches_only_demanded_branch() {
        let m = MachineId(Uuid::new_v4());
        let root = NodeId(Uuid::new_v4());
        let branch_a = NodeId(Uuid::new_v4());
        let branch_b = NodeId(Uuid::new_v4());
        let consumer_a = NodeId(Uuid::new_v4());

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(counted_node(
                root,
                "root",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![],
            ))
            .unwrap()
            .add_node(counted_node(
                branch_a,
                "branch-a",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![root],
            ))
            .unwrap()
            .add_node(counted_node(
                branch_b,
                "branch-b",
                Targets::Machine(m),
                RunPolicy::Lazy,
                vec![root],
            ))
            .unwrap()
            .add_node(counted_node(
                consumer_a,
                "consumer-a",
                Targets::Machine(m),
                RunPolicy::Always,
                vec![branch_a],
            ))
            .unwrap();

        let executor = Arc::new(CountingExecutor::default());
        let report = run_counted(&infra, Arc::clone(&executor)).await;
        let entry = |id| report.entries.iter().find(|e| e.node_id == id).unwrap();

        // Shared head + demanded branch + consumer ran.
        for (id, name) in [
            (root, "root"),
            (branch_a, "branch-a"),
            (consumer_a, "consumer-a"),
        ] {
            assert_eq!(executor.count(name), 1, "{name} should run");
            assert_eq!(entry(id).status, NodeStatus::Changed);
        }
        // The branch with no consumer stays dormant even though `root` changed.
        assert_eq!(executor.count("branch-b"), 0);
        assert_eq!(entry(branch_b).status, NodeStatus::Skipped);
        assert_eq!(entry(branch_b).message.as_deref(), Some("not demanded"));
    }

    #[tokio::test]
    async fn edge_readiness_runs_deps_first() {
        let m = MachineId(Uuid::new_v4());
        let n1 = NodeId(Uuid::new_v4());
        let n2 = NodeId(Uuid::new_v4());

        let mut n2node = shell_node(
            n2,
            "second",
            ShellOp::run(argv!["sh", "-c", "echo second"]),
            Targets::Machine(m),
        );
        n2node.deps = vec![n1];

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(shell_node(
                n1,
                "first",
                ShellOp::run(argv!["sh", "-c", "echo first"]),
                Targets::Machine(m),
            ))
            .unwrap()
            .add_node(n2node)
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let vault = Arc::new(VaultSession::default());

        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel,
                vault,
                executor: Arc::new(LocalExecutor),
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();

        assert_eq!(report.entries.len(), 2);
        let first = report
            .entries
            .iter()
            .find(|e| e.node_name == "first")
            .unwrap();
        let second = report
            .entries
            .iter()
            .find(|e| e.node_name == "second")
            .unwrap();
        assert_eq!(first.status, NodeStatus::Changed);
        assert_eq!(second.status, NodeStatus::Changed);
    }

    #[tokio::test]
    async fn output_change_policy_can_skip_downstream_nodes() {
        let m = MachineId(Uuid::new_v4());
        let upgrade = NodeId(Uuid::new_v4());
        let reboot = NodeId(Uuid::new_v4());

        let mut upgrade_node = shell_node(
            upgrade,
            "upgrade",
            ShellOp::run(argv!["sh", "-c", "printf '0 upgraded\\n'"]),
            Targets::Machine(m),
        );
        upgrade_node.policy.success.change_policy.rules.push(
            OutputChangeRule::unchanged_when_contains(OutputMatchStream::Stdout, "0 upgraded"),
        );

        let mut reboot_node = shell_node(
            reboot,
            "reboot",
            ShellOp::run(argv!["sh", "-c", "printf reboot"]),
            Targets::Machine(m),
        );
        reboot_node.deps = vec![upgrade];

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(upgrade_node)
            .unwrap()
            .add_node(reboot_node)
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);

        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::default()),
                executor: Arc::new(LocalExecutor),
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();

        let upgrade_entry = report
            .entries
            .iter()
            .find(|entry| entry.node_id == upgrade)
            .unwrap();
        let reboot_entry = report
            .entries
            .iter()
            .find(|entry| entry.node_id == reboot)
            .unwrap();
        assert_eq!(upgrade_entry.status, NodeStatus::Unchanged);
        assert_eq!(reboot_entry.status, NodeStatus::Skipped);
    }

    #[tokio::test]
    async fn skipped_nodes_satisfy_downstream_dependencies() {
        let m = MachineId(Uuid::new_v4());
        let seed = NodeId(Uuid::new_v4());
        let probe = NodeId(Uuid::new_v4());
        let conditional = NodeId(Uuid::new_v4());
        let after_skip = NodeId(Uuid::new_v4());

        let mut probe_node = shell_node(
            probe,
            "probe",
            ShellOp::run(argv!["sh", "-c", "printf 'unchanged\\n'"]),
            Targets::Machine(m),
        );
        probe_node.deps = vec![seed];
        probe_node.policy.success.change_policy.rules.push(
            OutputChangeRule::unchanged_when_contains(OutputMatchStream::Stdout, "unchanged"),
        );

        let mut conditional_node = shell_node(
            conditional,
            "conditional",
            ShellOp::run(argv!["sh", "-c", "printf conditional"]),
            Targets::Machine(m),
        );
        conditional_node.deps = vec![probe];

        let mut after_skip_node = shell_node(
            after_skip,
            "after-skip",
            ShellOp::run(argv!["sh", "-c", "printf after-skip"]),
            Targets::Machine(m),
        );
        after_skip_node.deps = vec![conditional];
        after_skip_node.policy.run_policy = RunPolicy::Always;

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(shell_node(
                seed,
                "seed",
                ShellOp::run(argv!["sh", "-c", "printf seed"]),
                Targets::Machine(m),
            ))
            .unwrap()
            .add_node(probe_node)
            .unwrap()
            .add_node(conditional_node)
            .unwrap()
            .add_node(after_skip_node)
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);

        let report = tokio::time::timeout(
            Duration::from_secs(2),
            DefaultScheduler.run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::default()),
                executor: Arc::new(LocalExecutor),
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            }),
        )
        .await
        .expect("scheduler should not deadlock behind a skipped dependency")
        .unwrap();

        let entry = |node_id| {
            report
                .entries
                .iter()
                .find(|entry| entry.node_id == node_id)
                .unwrap()
        };
        assert_eq!(entry(seed).status, NodeStatus::Changed);
        assert_eq!(entry(probe).status, NodeStatus::Unchanged);
        assert_eq!(entry(conditional).status, NodeStatus::Skipped);
        assert_eq!(entry(after_skip).status, NodeStatus::Changed);
    }

    #[tokio::test]
    async fn output_change_policy_can_mark_stderr_as_changed() {
        let m = MachineId(Uuid::new_v4());
        let n = NodeId(Uuid::new_v4());

        let mut node = shell_node(
            n,
            "stderr-change",
            ShellOp::run(argv!["sh", "-c", "printf 'pkg changed\\n' >&2"]),
            Targets::Machine(m),
        );
        node.policy
            .success
            .change_policy
            .rules
            .push(OutputChangeRule::changed_when_contains(
                OutputMatchStream::Stderr,
                "pkg changed",
            ));
        node.policy
            .success
            .change_policy
            .rules
            .push(OutputChangeRule::unchanged_when_contains(
                OutputMatchStream::Any,
                "pkg",
            ));

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(node)
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);

        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::default()),
                executor: Arc::new(LocalExecutor),
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].status, NodeStatus::Changed);
    }

    #[tokio::test]
    async fn reconnect_waits_until_probe_succeeds() {
        let m = MachineId(Uuid::new_v4());
        let node = shell_node(
            NodeId(Uuid::new_v4()),
            "reconnect",
            ShellOp::run(argv!["true"]),
            Targets::Machine(m),
        );
        let executor = ReconnectProbeExecutor::new(1, 1);
        let (events, _) = tokio::sync::broadcast::channel(16);

        wait_for_reconnect(
            &executor,
            &events,
            &node,
            m,
            ReconnectConfig {
                max_attempts: 4,
                backoff: Backoff::Fixed(Duration::ZERO),
            },
            None,
            "test",
        )
        .await
        .unwrap();

        assert_eq!(executor.reconnect_calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn reboot_confirmed_only_after_boot_id_changes_then_ready() {
        let m = MachineId(Uuid::new_v4());
        let mut node = shell_node(
            NodeId(Uuid::new_v4()),
            "reboot",
            ShellOp::run(argv!["reboot"]),
            Targets::Machine(m),
        );
        node.policy.post_run = PostRunPolicy::ExpectReboot {
            readiness_check: Some(ShellOp::run(argv!["readiness"])),
        };

        // The first two boot-id probes still report the pre-reboot id (the host
        // hasn't gone down yet) — those reconnects must be rejected. The third
        // reports a new id, proving a real reboot.
        let executor = RebootProbeExecutor::new(vec!["boot-A", "boot-A", "boot-B"], 2, 0);
        let (events, _) = tokio::sync::broadcast::channel(32);

        wait_for_reboot(
            &executor,
            &events,
            &node,
            m,
            &Some("boot-A".to_string()),
            false,
            ReconnectConfig {
                max_attempts: 50,
                backoff: Backoff::Fixed(Duration::ZERO),
            },
        )
        .await
        .unwrap();

        // Kept probing past the unchanged boot ids instead of falsely accepting
        // the still-live host (the __INFRAZEUG_UNCHANGED__-era reboot race).
        assert!(
            executor.boot_probe_calls.load(Ordering::SeqCst) >= 3,
            "must not accept a reconnect while the boot id is unchanged"
        );
        // Readiness gate polled until it passed (2 failures + 1 success).
        assert_eq!(executor.readiness_calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn reboot_without_boot_id_baseline_requires_observed_disconnect() {
        let m = MachineId(Uuid::new_v4());
        let mut node = shell_node(
            NodeId(Uuid::new_v4()),
            "reboot",
            ShellOp::run(argv!["reboot"]),
            Targets::Machine(m),
        );
        node.policy.post_run = PostRunPolicy::ExpectReboot {
            readiness_check: None,
        };

        // No baseline boot id: the host must actually drop (one failed reconnect)
        // before a later reconnect is accepted as "rebooted".
        let executor = RebootProbeExecutor::new(vec!["boot-X"], 0, 1);
        let (events, _) = tokio::sync::broadcast::channel(32);

        wait_for_reboot(
            &executor,
            &events,
            &node,
            m,
            &None,
            false,
            ReconnectConfig {
                max_attempts: 50,
                backoff: Backoff::Fixed(Duration::ZERO),
            },
        )
        .await
        .unwrap();

        assert_eq!(executor.reconnect_calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn expected_reboot_syncs_graph_state_before_global_lock_release() {
        let m = MachineId(Uuid::new_v4());
        let first = NodeId(Uuid::new_v4());
        let reboot = NodeId(Uuid::new_v4());

        let mut reboot_node = shell_node(
            reboot,
            "reboot",
            ShellOp::run(argv!["reboot"]),
            Targets::Machine(m),
        );
        reboot_node.deps = vec![first];
        reboot_node.policy.post_run = PostRunPolicy::ExpectReboot {
            readiness_check: None,
        };
        reboot_node.policy.locks.global_locks = vec!["pkg-manager".into()];

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(shell_node(
                first,
                "first",
                ShellOp::run(argv!["true"]),
                Targets::Machine(m),
            ))
            .unwrap()
            .add_node(reboot_node)
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let (sync_seen_tx, sync_seen_rx) = oneshot::channel();
        let (sync_release_tx, sync_release_rx) = oneshot::channel();
        let executor = Arc::new(BlockingGraphSyncExecutor::new(
            reboot,
            sync_seen_tx,
            sync_release_rx,
        ));

        let run = tokio::spawn(async move {
            DefaultScheduler
                .run(SchedRuntime {
                    infra: &infra,
                    plan,
                    limits: GlobalLimits::default(),
                    events,
                    commands: cmd_rx,
                    interact: Arc::new(NoPromptInteractor),
                    cancel: CancellationToken::new(),
                    vault: Arc::new(VaultSession::default()),
                    executor,
                    native_executor: empty_native_executor(),
                    hash_relay: None,
                    captures: Arc::new(CaptureStore::new()),
                    capture_spill_root: None,
                })
                .await
        });

        let synced = sync_seen_rx.await.expect("graph sync should be attempted");
        assert!(
            synced
                .iter()
                .any(|(node, status)| *node == first && *status == NodeStatus::Changed),
            "previous successful node should be replayed to the reconnected agent"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !run.is_finished(),
            "scheduler completed before graph sync returned"
        );

        let _ = sync_release_tx.send(());
        let report = run.await.unwrap().unwrap();
        assert_eq!(report.entries.len(), 2);
        assert!(report
            .entries
            .iter()
            .all(|e| e.status == NodeStatus::Changed));
    }

    #[tokio::test]
    async fn expect_shutdown_reconnects_after_nonzero_exit() {
        let m = MachineId(Uuid::new_v4());
        let reboot = NodeId(Uuid::new_v4());

        let mut reboot_node = shell_node(
            reboot,
            "reboot",
            ShellOp::run(argv!["reboot-exit-255"]),
            Targets::Machine(m),
        );
        reboot_node.policy.post_run = PostRunPolicy::ExpectReboot {
            readiness_check: None,
        };

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(reboot_node)
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let executor = Arc::new(NonZeroExpectedShutdownExecutor {
            reconnect_calls: AtomicUsize::new(0),
        });
        let executor_seen = Arc::clone(&executor);

        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::default()),
                executor,
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].status, NodeStatus::Changed);
        assert_eq!(executor_seen.reconnect_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cross_machine_read_then_write_local() {
        let remote = MachineId(Uuid::new_v4());
        let local = MachineId(Uuid::new_v4());
        let n_seed = NodeId(Uuid::new_v4());
        let n_fetch = NodeId(Uuid::new_v4());
        let n_save = NodeId(Uuid::new_v4());

        let dir = tempdir().unwrap();
        let remote_path = dir.path().join("remote.txt");
        let local_path = dir.path().join("local.txt");

        let mut save = shell_node(
            n_save,
            "save-local",
            ShellOp::write_file(
                &local_path,
                FileSource::capture_on_machine(n_fetch.0, remote.0),
                0o644,
            ),
            Targets::Machine(local),
        );
        save.deps = vec![n_fetch];

        let infra = Infra::new()
            .add_machine(local_machine(remote, "remote"))
            .unwrap()
            .add_machine(local_machine(local, "local"))
            .unwrap()
            .add_node(shell_node(
                n_seed,
                "seed-remote",
                ShellOp::write_file_bytes(&remote_path, b"from-remote", 0o644),
                Targets::Machine(remote),
            ))
            .unwrap()
            .add_node({
                let mut fetch = shell_node(
                    n_fetch,
                    "fetch-remote",
                    ShellOp::read_file(&remote_path),
                    Targets::Machine(remote),
                );
                fetch.deps = vec![n_seed];
                fetch
            })
            .unwrap()
            .add_node(save)
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let vault = Arc::new(VaultSession::default());

        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel,
                vault,
                executor: Arc::new(LocalExecutor),
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();

        assert_eq!(report.entries.len(), 3);
        assert!(report
            .entries
            .iter()
            .all(|e| e.status == NodeStatus::Changed));
        assert_eq!(std::fs::read(&local_path).unwrap(), b"from-remote");
    }

    /// Regression: when the host that produces a capture is unreachable, the
    /// local consumer of that capture must cascade-*block* (Skipped, "blocked by
    /// upstream") rather than run and fail with a cryptic "capture missing".
    ///
    /// Mirrors a real apply where `connect/<host>` fails (SSH down): the on-host
    /// producer is blocked, and a controller-side consumer used to run anyway and
    /// fail because the producer never wrote its capture.
    #[tokio::test]
    async fn unreachable_capture_producer_blocks_consumer_instead_of_capture_missing() {
        let remote = MachineId(Uuid::new_v4());
        let local = MachineId(Uuid::new_v4());
        let n_probe = NodeId(Uuid::new_v4());
        let n_fetch = NodeId(Uuid::new_v4());
        let n_save = NodeId(Uuid::new_v4());

        let dir = tempdir().unwrap();
        let remote_path = dir.path().join("remote.txt");
        let local_path = dir.path().join("local.txt");

        // Consumer on the controller writes the remote capture to a local file.
        let mut save = shell_node(
            n_save,
            "save-local",
            ShellOp::write_file(
                &local_path,
                FileSource::capture_on_machine(n_fetch.0, remote.0),
                0o644,
            ),
            Targets::Machine(local),
        );
        save.deps = vec![n_fetch];
        // `Always` so the consumer would genuinely run (and hit "capture missing")
        // were it not for the cascade-block — that is the regression under test.
        save.policy.run_policy = RunPolicy::Always;

        let infra = Infra::new()
            .add_machine(local_machine(remote, "remote"))
            .unwrap()
            .add_machine(local_machine(local, "local"))
            .unwrap()
            // Stand-in for the host's connect/reachability step: it runs and fails,
            // so the producer downstream is *blocked* (skipped, never produces a
            // capture) rather than failing itself.
            .add_node(shell_node(
                n_probe,
                "probe-remote",
                ShellOp::run(vec!["probe".into()]),
                Targets::Machine(remote),
            ))
            .unwrap()
            .add_node({
                let mut fetch = shell_node(
                    n_fetch,
                    "fetch-remote",
                    ShellOp::read_file(&remote_path),
                    Targets::Machine(remote),
                );
                fetch.deps = vec![n_probe];
                fetch.policy.run_policy = RunPolicy::Always;
                fetch
            })
            .unwrap()
            .add_node(save)
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);

        // Every op on `remote` fails — the remote host is unreachable, so its
        // connect probe (and thus the producer) is blocked.
        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::default()),
                executor: Arc::new(SelectiveFailExecutor { fail: remote }),
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();

        let entry = |id: NodeId| report.entries.iter().find(|e| e.node_id == id).unwrap();

        // The probe (connect stand-in) failed; the producer is therefore *blocked*
        // — skipped without running, so it never produced its capture.
        assert_eq!(entry(n_probe).status, NodeStatus::Failed);
        assert_eq!(entry(n_fetch).status, NodeStatus::Skipped);

        // The consumer cascade-blocks instead of failing with "capture missing".
        let save_entry = entry(n_save);
        assert_eq!(
            save_entry.status,
            NodeStatus::Skipped,
            "consumer should block, got {:?} ({:?})",
            save_entry.status,
            save_entry.message
        );
        assert_eq!(save_entry.message.as_deref(), Some("blocked by upstream"));
        assert!(report.entries.iter().all(|e| !e
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("capture missing")));
        // The consumer never ran, so it wrote nothing locally.
        assert!(!local_path.exists());
    }

    #[tokio::test]
    async fn cancel_machine_command_skips_pending_work() {
        let m = MachineId(Uuid::new_v4());
        let n = NodeId(Uuid::new_v4());

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(shell_node(
                n,
                "work",
                ShellOp::run(argv!["sh", "-c", "echo hi"]),
                Targets::Machine(m),
            ))
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (tx, cmd_rx) = mpsc::channel(4);
        // Pre-queue the cancel so it's drained before the node is dispatched.
        tx.send(SchedCommand::CancelMachine { machine: m })
            .await
            .unwrap();
        let cancel = CancellationToken::new();
        let vault = Arc::new(VaultSession::default());

        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel,
                vault,
                executor: Arc::new(LocalExecutor),
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].status, NodeStatus::Cancelled);
    }

    #[tokio::test]
    async fn cancel_node_command_kills_in_flight_work() {
        let m = MachineId(Uuid::new_v4());
        let n = NodeId(Uuid::new_v4());

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(shell_node(
                n,
                "slow",
                ShellOp::run(argv!["sh", "-c", "sleep 30"]),
                Targets::Machine(m),
            ))
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (tx, cmd_rx) = mpsc::channel(4);
        // Fire the cancel once the unit is in-flight; grace 0 = immediate kill.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            let _ = tx
                .send(SchedCommand::CancelNode {
                    node: n,
                    machine: m,
                    grace: Duration::ZERO,
                })
                .await;
        });
        let cancel = CancellationToken::new();
        let vault = Arc::new(VaultSession::default());

        let report = tokio::time::timeout(
            Duration::from_secs(5),
            DefaultScheduler.run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel,
                vault,
                executor: Arc::new(LocalExecutor),
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            }),
        )
        .await
        .expect("scheduler did not honor in-flight cancel within 5s")
        .unwrap();

        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].status, NodeStatus::Cancelled);
    }

    #[tokio::test]
    async fn replay_reruns_failed_fail_fast_node() {
        let m = MachineId(Uuid::new_v4());
        let flaky = NodeId(Uuid::new_v4());
        let slow = NodeId(Uuid::new_v4());

        let dir = tempdir().unwrap();
        let marker = dir.path().join("attempted");
        // First run: leave the marker and fail. Replayed run: marker exists, succeed.
        let script = format!(
            "if [ -e {p} ]; then exit 0; else touch {p}; exit 1; fi",
            p = marker.display()
        );

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(shell_node(
                flaky,
                "flaky",
                ShellOp::run(argv!["sh", "-c", &script]),
                Targets::Machine(m),
            ))
            .unwrap()
            // Independent long unit keeps the run loop alive across the replay.
            .add_node(shell_node(
                slow,
                "slow",
                ShellOp::run(argv!["sh", "-c", "sleep 2"]),
                Targets::Machine(m),
            ))
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, mut events_rx) = tokio::sync::broadcast::channel(64);
        let (tx, cmd_rx) = mpsc::channel(4);
        // Replay the flaky unit as soon as its failure is reported.
        tokio::spawn(async move {
            while let Ok(ev) = events_rx.recv().await {
                if let SchedEvent::NodeFinished {
                    node,
                    status: NodeStatus::Failed,
                    ..
                } = ev
                {
                    if node == flaky {
                        let _ = tx
                            .send(SchedCommand::ReplayNode {
                                node: flaky,
                                machine: m,
                            })
                            .await;
                        break;
                    }
                }
            }
        });
        let cancel = CancellationToken::new();
        let vault = Arc::new(VaultSession::default());

        let report = tokio::time::timeout(
            Duration::from_secs(10),
            DefaultScheduler.run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel,
                vault,
                executor: Arc::new(LocalExecutor),
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            }),
        )
        .await
        .expect("scheduler did not finish replay run within 10s")
        .unwrap();

        let flaky_entry = report
            .entries
            .iter()
            .find(|e| e.node_id == flaky)
            .expect("flaky node missing from report");
        assert!(
            matches!(
                flaky_entry.status,
                NodeStatus::Changed | NodeStatus::Unchanged
            ),
            "replayed node should succeed, got {:?} ({:?})",
            flaky_entry.status,
            flaky_entry.message
        );
    }

    #[tokio::test]
    async fn independent_machines_run_in_parallel() {
        let m1 = MachineId(Uuid::new_v4());
        let m2 = MachineId(Uuid::new_v4());
        let n1 = NodeId(Uuid::new_v4());
        let n2 = NodeId(Uuid::new_v4());

        let infra = Infra::new()
            .add_machine(local_machine(m1, "host-a"))
            .unwrap()
            .add_machine(local_machine(m2, "host-b"))
            .unwrap()
            .add_node(shell_node(
                n1,
                "sleep-a",
                ShellOp::run(argv!["sh", "-c", "sleep 0.15"]),
                Targets::Machine(m1),
            ))
            .unwrap()
            .add_node(shell_node(
                n2,
                "sleep-b",
                ShellOp::run(argv!["sh", "-c", "sleep 0.15"]),
                Targets::Machine(m2),
            ))
            .unwrap();

        let plan = infra.plan().unwrap();
        let (events, _) = tokio::sync::broadcast::channel(32);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let vault = Arc::new(VaultSession::default());

        let started = Instant::now();
        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel,
                vault,
                executor: Arc::new(LocalExecutor),
                native_executor: empty_native_executor(),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();
        let elapsed = started.elapsed();

        assert_eq!(report.entries.len(), 2);
        assert!(report
            .entries
            .iter()
            .all(|e| matches!(e.status, NodeStatus::Changed | NodeStatus::Unchanged)));
        // Serial execution would be ~300ms+; parallel should finish closer to 150ms.
        assert!(
            elapsed < Duration::from_millis(280),
            "expected parallel machine execution, took {:?}",
            elapsed
        );
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct DiscoverTwo;

    #[async_trait]
    impl NodeMethod for DiscoverTwo {
        type Input = ();
        type Output = Vec<crate::machine::DiscoveredMachine>;

        fn name(&self) -> &'static str {
            "test.discover_two"
        }

        async fn execute(
            &self,
            _ctx: &NodeCtx,
            _input: (),
        ) -> infrazeug_native::Result<NativeResult> {
            let machines: Vec<crate::machine::DiscoveredMachine> = ["w0", "w1"]
                .iter()
                .map(|n| crate::machine::DiscoveredMachine {
                    name: (*n).to_string(),
                    ssh: crate::machine::SshConfig::new("127.0.0.1"),
                    vars: Default::default(),
                    tags: Vec::new(),
                    os: None,
                })
                .collect();
            NativeResult::changed("discovered 2")
                .with_json_capture(&machines)
                .map_err(|e| NativeError::other(e.to_string()))
        }
    }

    #[tokio::test]
    async fn dynamic_group_fans_out_per_machine() {
        let controller = MachineId(Uuid::from_u128(0xC0));
        let disc = NodeId(Uuid::from_u128(0xD15C));
        let tmpl_connect = NodeId(Uuid::from_u128(0xC04EC7));
        let tmpl_step = NodeId(Uuid::from_u128(0x57E9));

        let placeholder = Targets::Machine(crate::dynamic::template_placeholder_machine());
        let template = vec![
            NodeBuilder::connect(tmpl_connect, placeholder.clone())
                .name("connect")
                .build(),
            NodeBuilder::shell(tmpl_step, ShellOp::run(argv!["true"]), placeholder.clone())
                .name("step")
                .deps(vec![tmpl_connect])
                .build(),
        ];

        let mut infra = Infra::new()
            .add_machine(local_machine(controller, "controller"))
            .unwrap()
            .add_node(
                NodeBuilder::native_with_input(
                    disc,
                    "test.discover_two",
                    serde_cbor::Value::Null,
                    Targets::Machine(controller),
                )
                .name("discover")
                .run_policy(RunPolicy::Always)
                .build(),
            )
            .unwrap()
            .add_node(barrier_node(
                crate::dynamic::dyn_exit_node_id("workers"),
                "workers/exit",
                Targets::Machine(controller),
                vec![disc],
            ))
            .unwrap();
        infra.push_dynamic_group(crate::dynamic::DynamicGroup {
            label: "workers".into(),
            discovery_node: disc,
            template,
            template_entry_deps: vec![disc],
            fail_policy: FailPolicy::Tolerate {
                max_failed: usize::MAX,
            },
            max_parallel_machines: None,
        });

        let mut registry = MethodRegistry::new();
        registry.register(DiscoverTwo);

        let plan = infra.plan().unwrap();
        let (events, mut rx) = tokio::sync::broadcast::channel(256);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::default()),
                executor: Arc::new(LocalExecutor),
                native_executor: Arc::new(LocalNativeExecutor::new(Arc::new(registry))),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();

        let ok = |id: NodeId| {
            report.entries.iter().any(|e| {
                e.node_id == id && matches!(e.status, NodeStatus::Changed | NodeStatus::Unchanged)
            })
        };

        assert!(ok(disc), "discovery node should have run");

        // Both discovered machines' connect + step instances ran successfully.
        for name in ["w0", "w1"] {
            let mid = crate::dynamic::dyn_machine_id("workers", name);
            assert!(
                ok(crate::dynamic::dyn_instance_node_id(tmpl_connect, mid)),
                "connect instance for {name} missing/failed"
            );
            assert!(
                ok(crate::dynamic::dyn_instance_node_id(tmpl_step, mid)),
                "step instance for {name} missing/failed"
            );
        }

        // Exit barrier joined the whole fan-out.
        assert!(
            ok(crate::dynamic::dyn_exit_node_id("workers")),
            "exit barrier"
        );

        // A UnitsAdded event was emitted for the fan-out.
        let mut units_added = 0usize;
        while let Ok(ev) = rx.try_recv() {
            if let SchedEvent::UnitsAdded { added_units, .. } = ev {
                units_added += added_units;
            }
        }
        assert!(
            units_added >= 4,
            "expected >=4 added units, got {units_added}"
        );
    }

    /// Runs the per-machine template but fails every op on one specific machine.
    struct SelectiveFailExecutor {
        fail: MachineId,
    }

    #[async_trait]
    impl OpExecutor for SelectiveFailExecutor {
        async fn execute(
            &self,
            machine_id: MachineId,
            op: &ShellOp,
        ) -> infrazeug_shell::Result<ExecOutput> {
            if machine_id == self.fail {
                return Ok(ExecOutput {
                    exit_code: 1,
                    stdout: Vec::new(),
                    stderr: b"boom".to_vec(),
                });
            }
            infrazeug_shell::local::LocalShellExecutor::new()
                .execute(op)
                .await
        }
    }

    #[tokio::test]
    async fn dynamic_group_tolerates_a_failed_machine() {
        let controller = MachineId(Uuid::from_u128(0xC1));
        let disc = NodeId(Uuid::from_u128(0xD2));
        let tmpl_connect = NodeId(Uuid::from_u128(0xC0));
        let tmpl_step = NodeId(Uuid::from_u128(0x57));

        let placeholder = Targets::Machine(crate::dynamic::template_placeholder_machine());
        let template = vec![
            NodeBuilder::connect(tmpl_connect, placeholder.clone())
                .name("connect")
                .build(),
            NodeBuilder::shell(tmpl_step, ShellOp::run(argv!["true"]), placeholder.clone())
                .name("step")
                .deps(vec![tmpl_connect])
                .build(),
        ];

        // Exit barrier carries the group's (tolerate) fail policy.
        let mut exit = barrier_node(
            crate::dynamic::dyn_exit_node_id("workers"),
            "workers/exit",
            Targets::Machine(controller),
            vec![disc],
        );
        exit.policy.fail_policy = FailPolicy::Tolerate {
            max_failed: usize::MAX,
        };

        let mut infra = Infra::new()
            .add_machine(local_machine(controller, "controller"))
            .unwrap()
            .add_node(
                NodeBuilder::native_with_input(
                    disc,
                    "test.discover_two",
                    serde_cbor::Value::Null,
                    Targets::Machine(controller),
                )
                .name("discover")
                .run_policy(RunPolicy::Always)
                .build(),
            )
            .unwrap()
            .add_node(exit)
            .unwrap();
        infra.push_dynamic_group(crate::dynamic::DynamicGroup {
            label: "workers".into(),
            discovery_node: disc,
            template,
            template_entry_deps: vec![disc],
            fail_policy: FailPolicy::Tolerate {
                max_failed: usize::MAX,
            },
            max_parallel_machines: None,
        });

        let mut registry = MethodRegistry::new();
        registry.register(DiscoverTwo);

        // Fail every op on w0; w1 succeeds.
        let w0 = crate::dynamic::dyn_machine_id("workers", "w0");
        let w1 = crate::dynamic::dyn_machine_id("workers", "w1");

        let plan = infra.plan().unwrap();
        let (events, _rx) = tokio::sync::broadcast::channel(256);
        let (_tx, cmd_rx) = mpsc::channel(4);
        let report = DefaultScheduler
            .run(SchedRuntime {
                infra: &infra,
                plan,
                limits: GlobalLimits::default(),
                events,
                commands: cmd_rx,
                interact: Arc::new(NoPromptInteractor),
                cancel: CancellationToken::new(),
                vault: Arc::new(VaultSession::default()),
                executor: Arc::new(SelectiveFailExecutor { fail: w0 }),
                native_executor: Arc::new(LocalNativeExecutor::new(Arc::new(registry))),
                hash_relay: None,
                captures: Arc::new(CaptureStore::new()),
                capture_spill_root: None,
            })
            .await
            .unwrap();

        let status = |id: NodeId| {
            report
                .entries
                .iter()
                .find(|e| e.node_id == id)
                .map(|e| e.status)
        };

        // w0's connect failed (every op fails on w0).
        assert_eq!(
            status(crate::dynamic::dyn_instance_node_id(tmpl_connect, w0)),
            Some(NodeStatus::Failed)
        );
        // w1 succeeded end to end.
        assert!(matches!(
            status(crate::dynamic::dyn_instance_node_id(tmpl_step, w1)),
            Some(NodeStatus::Changed | NodeStatus::Unchanged)
        ));
        // The tolerate exit barrier still joined (run not blocked by w0's failure).
        assert!(matches!(
            status(crate::dynamic::dyn_exit_node_id("workers")),
            Some(NodeStatus::Changed | NodeStatus::Unchanged)
        ));
    }
}
