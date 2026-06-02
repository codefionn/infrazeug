use crate::error::{CoreError, Result};
use crate::hash_relay::HashRelay;
use crate::id::{GroupId, MachineId, NodeId, RunId};
use crate::interactor::Interactor;
use crate::limits::GlobalLimits;
use crate::lint::LintReport;
use crate::machine::{Group, Machine, MachineKind, SshConfig};
use crate::native_exec::NativeExecutor;
use crate::node::{Node, NodeBody, NodeBuilder, PlanOutcome, RunPolicy, Targets};
use crate::plan::{
    map_plan_outcome, node_fingerprint, plan_digest, Plan, PlannedNode, Preview, PreviewNode,
};
use crate::report::RunReport;
use crate::runtime::{RunGuard, RunMode, RuntimeConfig, VaultSession};
use crate::scheduler::{DefaultScheduler, SchedRuntime, Scheduler};
use crate::slice::{slice_to_plan, PlanSlice, SliceStep};
use crate::transport::TransportChoice;
use crate::varset::VarSet;
use infrazeug_native::MethodRegistry;
use infrazeug_secrets::{FsBackend, VaultStore};
use infrazeug_shell::ShellOp;
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone)]
pub struct Infra {
    pub global_vars: VarSet,
    pub groups: Vec<Group>,
    pub machines: Vec<Machine>,
    pub nodes: Vec<Node>,
    pub runtime: RuntimeConfig,
    pub limits: GlobalLimits,
    /// Data-key names to unlock interactively at apply start (modal).
    pub vault_data_keys: Vec<String>,
    /// Per-machine transport override at apply time.
    pub transport_choices: std::collections::HashMap<MachineId, TransportChoice>,
    /// Default for `MachineKind::Remote` when not in `transport_choices`.
    pub default_remote_transport: TransportChoice,
    /// Dynamic machine groups expanded at apply time (see [`crate::dynamic`]).
    pub dynamic_groups: Vec<crate::dynamic::DynamicGroup>,
    scheduler: Arc<dyn Scheduler>,
}

impl Default for Infra {
    fn default() -> Self {
        Self::new()
    }
}

impl Infra {
    pub fn new() -> Self {
        Self {
            global_vars: VarSet::new(),
            groups: Vec::new(),
            machines: Vec::new(),
            nodes: Vec::new(),
            runtime: RuntimeConfig::default(),
            limits: GlobalLimits::default(),
            vault_data_keys: Vec::new(),
            transport_choices: std::collections::HashMap::new(),
            default_remote_transport: TransportChoice::SshAgentPush,
            dynamic_groups: Vec::new(),
            scheduler: Arc::new(DefaultScheduler),
        }
    }

    /// Register a dynamic machine group (discovery node + per-machine template).
    pub fn push_dynamic_group(&mut self, group: crate::dynamic::DynamicGroup) {
        self.dynamic_groups.push(group);
    }

    pub fn with_transport_choice(mut self, machine_id: MachineId, choice: TransportChoice) -> Self {
        self.transport_choices.insert(machine_id, choice);
        self
    }

    pub fn with_default_remote_transport(mut self, choice: TransportChoice) -> Self {
        self.default_remote_transport = choice;
        self
    }

    pub fn transport_for_machine(&self, machine: &Machine) -> TransportChoice {
        if let Some(c) = self.transport_choices.get(&machine.id) {
            return *c;
        }
        match machine.kind {
            MachineKind::Local | MachineKind::Container(_) => TransportChoice::Local,
            MachineKind::Remote { .. } => self.default_remote_transport,
        }
    }

    pub fn with_vault_data_keys(mut self, keys: Vec<String>) -> Self {
        self.vault_data_keys = keys;
        self
    }

    pub fn with_global_vars(mut self, vars: VarSet) -> Self {
        self.global_vars = vars;
        self
    }

    pub fn with_runtime(mut self, runtime: RuntimeConfig) -> Self {
        self.runtime = runtime;
        self
    }

    pub fn with_scheduler(mut self, scheduler: Arc<dyn Scheduler>) -> Self {
        self.scheduler = scheduler;
        self
    }

    pub fn add_group(mut self, group: Group) -> Result<Self> {
        if self.groups.iter().any(|g| g.name == group.name) {
            return Err(CoreError::DuplicateName {
                kind: "group",
                name: group.name.to_string(),
            });
        }
        self.groups.push(group);
        Ok(self)
    }

    pub fn add_machine(mut self, machine: Machine) -> Result<Self> {
        if self.machines.iter().any(|m| m.name == machine.name) {
            return Err(CoreError::DuplicateName {
                kind: "machine",
                name: machine.name.to_string(),
            });
        }
        self.machines.push(machine);
        Ok(self)
    }

    pub fn add_node(mut self, node: Node) -> Result<Self> {
        if self.nodes.iter().any(|n| n.name == node.name) {
            return Err(CoreError::DuplicateName {
                kind: "node",
                name: node.name.to_string(),
            });
        }
        self.nodes.push(node);
        Ok(self)
    }

    pub fn machine_by_name(&self, name: &str) -> Option<&Machine> {
        self.machines.iter().find(|m| m.name == name)
    }

    pub fn machine_by_id(&self, id: MachineId) -> Option<&Machine> {
        self.machines.iter().find(|m| m.id == id)
    }

    /// Fail-fast lint: aggregates every finding from [`lint_report`](Self::lint_report)
    /// into a single [`CoreError::Lint`] when any error is present.
    pub fn lint(&self) -> Result<()> {
        self.lint_report().into_result()
    }

    /// Collect *all* lint diagnostics in one pass (errors and warnings).
    ///
    /// Unlike [`lint`](Self::lint) this never short-circuits, so a caller can
    /// surface the full set of problems and their remediation at once.
    pub fn lint_report(&self) -> LintReport {
        self.lint_report_with_methods(None)
    }

    /// Like [`lint_report`](Self::lint_report) but validates native method ids
    /// against a playbook [`MethodRegistry`] when provided.
    pub fn lint_report_with_methods(&self, methods: Option<&MethodRegistry>) -> LintReport {
        let mut report = LintReport::new();
        self.collect_graph(&mut report);
        self.collect_lazy_demandability(&mut report);
        self.collect_transports(&mut report);
        if let Some(methods) = methods {
            self.collect_native_methods(&mut report, methods);
        }
        crate::capture::collect_capture_refs(self, &mut report);
        crate::secret_scan::collect_plaintext_secrets(self, &mut report);
        crate::test_mode::collect_like_configs(&self.machines, &mut report);
        self.collect_container_graph(&mut report);
        self.collect_dynamic_groups(&mut report, methods);
        report
    }

    fn collect_lazy_demandability(&self, report: &mut LintReport) {
        let policy_by_id: FxHashMap<NodeId, RunPolicy> = self
            .nodes
            .iter()
            .map(|node| (node.id, node.policy.run_policy))
            .collect();
        let mut dependents_by_id: FxHashMap<NodeId, Vec<NodeId>> = FxHashMap::default();
        for node in &self.nodes {
            for dep in &node.deps {
                dependents_by_id.entry(*dep).or_default().push(node.id);
            }
        }

        for node in &self.nodes {
            if !matches!(node.policy.run_policy, RunPolicy::Lazy) {
                continue;
            }
            if has_non_lazy_dependent(node.id, &policy_by_id, &dependents_by_id) {
                continue;
            }
            report.warning(
                CoreError::LazyNodeUndemandable {
                    node: node.name.clone(),
                },
                "add a non-lazy dependent, or remove the lazy node if it is intentionally dormant"
                    .to_string(),
            );
        }
    }

    fn collect_dynamic_groups(&self, report: &mut LintReport, methods: Option<&MethodRegistry>) {
        use std::collections::HashSet;
        for group in &self.dynamic_groups {
            match self.nodes.iter().find(|n| n.id == group.discovery_node) {
                Some(n) if matches!(n.body, NodeBody::Native { .. }) => {}
                Some(_) => report.error(
                    CoreError::other(format!(
                        "dynamic group `{}` discovery node `{}` is not a native node",
                        group.label, group.discovery_node
                    )),
                    "discovery must be a native method returning Vec<DiscoveredMachine>"
                        .to_string(),
                ),
                None => report.error(
                    CoreError::other(format!(
                        "dynamic group `{}` discovery node `{}` is missing from the graph",
                        group.label, group.discovery_node
                    )),
                    None,
                ),
            }
            if group.template.is_empty() {
                report.error(
                    CoreError::other(format!(
                        "dynamic group `{}` has an empty template",
                        group.label
                    )),
                    None,
                );
                continue;
            }
            let ids: HashSet<NodeId> = group.template.iter().map(|n| n.id).collect();
            if ids.len() != group.template.len() {
                report.error(
                    CoreError::other(format!(
                        "dynamic group `{}` has duplicate template node ids",
                        group.label
                    )),
                    None,
                );
            }
            let mut has_head = false;
            for n in &group.template {
                if !n.deps.iter().any(|d| ids.contains(d)) {
                    has_head = true;
                }
                for d in &n.deps {
                    if !ids.contains(d) {
                        report.error(
                            CoreError::other(format!(
                                "dynamic group `{}` template node `{}` depends on `{d}` which is not a template node",
                                group.label, n.name
                            )),
                            "external deps are wired through the discovery node at expansion; only reference other template nodes".to_string(),
                        );
                    }
                }
                if let (Some(methods), NodeBody::Native { method_id, .. }) = (methods, &n.body) {
                    if !methods.contains(method_id) {
                        report.error(
                            CoreError::other(format!(
                                "dynamic group `{}` template node `{}` uses unregistered method `{method_id}`",
                                group.label, n.name
                            )),
                            "register the method with `.method(..)` before building".to_string(),
                        );
                    }
                }
            }
            if !has_head {
                report.error(
                    CoreError::other(format!(
                        "dynamic group `{}` template has no head node (every node depends on another)",
                        group.label
                    )),
                    None,
                );
            }

            // Detect dependency cycles inside the template. The head-node check
            // above only catches a fully cyclic template; a partial cycle (a cycle
            // reachable from a head) would deadlock the fan-out at apply time.
            let name_by_id: FxHashMap<NodeId, &str> = group
                .template
                .iter()
                .map(|n| (n.id, n.name.as_str()))
                .collect();
            let mut graph = petgraph::Graph::<NodeId, ()>::new();
            let mut idx = FxHashMap::default();
            for n in &group.template {
                idx.insert(n.id, graph.add_node(n.id));
            }
            for n in &group.template {
                let to = idx[&n.id];
                for dep in &n.deps {
                    if let Some(from) = idx.get(dep) {
                        graph.add_edge(*from, to, ());
                    }
                }
            }
            report_graph_cycles(
                &graph,
                &name_by_id,
                &format!("dynamic group `{}` template", group.label),
                report,
            );
        }
    }

    pub fn lint_with_methods(&self, methods: &MethodRegistry) -> Result<()> {
        self.lint_report_with_methods(Some(methods)).into_result()
    }

    fn collect_container_graph(&self, report: &mut LintReport) {
        use infrazeug_emulate::graph::BuildGraph;
        let specs = crate::test_mode::specs_from_machines(&self.machines);
        if specs.is_empty() {
            return;
        }
        if let Err(e) = BuildGraph::from_specs(specs) {
            report.error(
                CoreError::other(format!("container build graph invalid: {e}")),
                "check `ContainerSpec` `from`/`copy_from` references for cycles or unknown stages"
                    .to_string(),
            );
        }
    }

    fn collect_transports(&self, report: &mut LintReport) {
        for node in &self.nodes {
            if !matches!(node.body, NodeBody::Native { .. }) {
                continue;
            }
            let machines = match self.resolve_targets(&node.targets) {
                Ok(m) => m,
                Err(e) => {
                    report.error(e, None);
                    continue;
                }
            };
            for mid in machines {
                let machine = self.machine_by_id(mid).expect("resolved");
                if matches!(machine.kind, MachineKind::Container(_)) {
                    report.error(
                        CoreError::NativeOnContainer {
                            node: node.name.clone(),
                            machine: machine.name.clone(),
                        },
                        "native methods on emulated containers are not supported in v1; target a Local machine or use ShellOp".to_string(),
                    );
                    continue;
                }
                if self.transport_for_machine(machine) == TransportChoice::SshAgentless {
                    report.error(
                        CoreError::NativeOnAgentless {
                            node: node.name.clone(),
                            machine: machine.name.clone(),
                        },
                        format!(
                            "native methods need the push agent; set a non-agentless transport for `{}` or rewrite the node as a ShellOp",
                            machine.name
                        ),
                    );
                }
            }
        }
    }

    fn collect_native_methods(&self, report: &mut LintReport, methods: &MethodRegistry) {
        for node in &self.nodes {
            let NodeBody::Native { method_id, .. } = &node.body else {
                continue;
            };
            let machines = match self.resolve_targets(&node.targets) {
                Ok(m) => m,
                Err(e) => {
                    report.error(e, None);
                    continue;
                }
            };
            for mid in machines {
                let machine = self.machine_by_id(mid).expect("resolved");
                if self.transport_for_machine(machine) != TransportChoice::Local {
                    continue;
                }
                if !methods.contains(method_id) {
                    report.error(
                        CoreError::NativeMethodNotRegistered {
                            node: node.name.clone(),
                            method: method_id.clone(),
                            machine: machine.name.clone(),
                        },
                        "register the method with `.register_method(..)` before building the node"
                            .to_string(),
                    );
                }
            }
        }
    }

    pub fn plan(&self) -> Result<Plan> {
        self.plan_with_methods(None)
    }

    pub fn plan_with_methods(&self, methods: Option<&MethodRegistry>) -> Result<Plan> {
        match methods {
            Some(methods) => self.lint_with_methods(methods)?,
            None => self.lint()?,
        }
        let mut planned = Vec::new();
        for node in &self.nodes {
            let machines = self.resolve_targets(&node.targets)?;
            planned.push(PlannedNode {
                node_id: node.id,
                name: node.name.to_string(),
                description: node.description.clone(),
                machines,
                outcome: PlanOutcome::Unknown,
                fingerprint: node_fingerprint(node),
            });
        }
        Ok(Plan {
            digest: plan_digest(&Plan {
                digest: crate::plan::PlanDigest([0; 32]),
                nodes: planned.clone(),
                signatures: Vec::new(),
            }),
            nodes: planned,
            signatures: Vec::new(),
        }
        .finalize())
    }

    /// Read-only dry-run that reports the real outcome of each node it can inspect.
    ///
    /// For native nodes targeting a `Local` (controller) machine this calls the
    /// method's [`plan`](infrazeug_native::NodeMethod::plan) via `native`, which
    /// observes live cloud state and returns create/reconcile (`Changed`) vs
    /// in-sync (`Unchanged`). Shell nodes and native nodes on remote transports are
    /// reported as `Unknown` (not previewable). Issues read-only API calls; never
    /// applies. The returned [`Preview`] is for display only — see its docs.
    pub async fn preview(&self, native: &dyn NativeExecutor) -> Result<Preview> {
        self.lint()?;
        let mut nodes = Vec::new();
        for node in &self.nodes {
            let machines = self.resolve_targets(&node.targets)?;
            let (outcome, previewable, note) = match &node.body {
                NodeBody::Native { method_id, input } => {
                    let mut previewed = (PlanOutcome::Unknown, false, None);
                    for mid in &machines {
                        let machine = self.machine_by_id(*mid).expect("resolved target");
                        if self.transport_for_machine(machine) == TransportChoice::Local {
                            // A failed read (auth/network) degrades just this node to
                            // `Unknown` with a note, so a preview never hard-fails.
                            // Read-only preview has no unlocked vault; credential-
                            // backed nodes report `Unknown` (see EnsureResource::plan).
                            previewed = match native
                                .plan_native(*mid, node.id, method_id, input, None)
                                .await
                            {
                                Ok(outcome) => (map_plan_outcome(outcome), true, None),
                                Err(e) => (PlanOutcome::Unknown, false, Some(e.to_string())),
                            };
                            break;
                        }
                    }
                    previewed
                }
                _ => (PlanOutcome::Unknown, false, None),
            };
            nodes.push(PreviewNode {
                node_id: node.id,
                name: node.name.to_string(),
                machines,
                outcome,
                previewable,
                note,
            });
        }
        Ok(Preview { nodes })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn apply(
        &self,
        plan: Plan,
        interact: Arc<dyn Interactor>,
        events: broadcast::Sender<crate::events::SchedEvent>,
        cancel: CancellationToken,
        commands_rx: mpsc::Receiver<crate::events::SchedCommand>,
        executor: Arc<dyn crate::exec::OpExecutor>,
        native_executor: Arc<dyn NativeExecutor>,
    ) -> Result<RunReport> {
        self.apply_with_relay(
            plan,
            interact,
            events,
            cancel,
            commands_rx,
            executor,
            native_executor,
            None,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn apply_slice(
        &self,
        slice: PlanSlice,
        interact: Arc<dyn Interactor>,
        events: broadcast::Sender<crate::events::SchedEvent>,
        cancel: CancellationToken,
        commands_rx: mpsc::Receiver<crate::events::SchedCommand>,
        executor: Arc<dyn crate::exec::OpExecutor>,
        native_executor: Arc<dyn NativeExecutor>,
        hash_relay: Arc<HashRelay>,
    ) -> Result<RunReport> {
        for step in &slice.steps {
            if let SliceStep::WaitForHash { id, expect, .. } = step {
                hash_relay.wait_for(*id, *expect).await;
            }
        }
        let plan = slice_to_plan(&slice);
        self.apply_with_relay(
            plan,
            interact,
            events,
            cancel,
            commands_rx,
            executor,
            native_executor,
            Some(hash_relay),
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn apply_with_relay(
        &self,
        plan: Plan,
        interact: Arc<dyn Interactor>,
        events: broadcast::Sender<crate::events::SchedEvent>,
        cancel: CancellationToken,
        commands_rx: mpsc::Receiver<crate::events::SchedCommand>,
        executor: Arc<dyn crate::exec::OpExecutor>,
        native_executor: Arc<dyn NativeExecutor>,
        hash_relay: Option<Arc<HashRelay>>,
        capture_spill_root: Option<std::path::PathBuf>,
    ) -> Result<RunReport> {
        let mut vault = self.build_vault_session();
        vault.unlock_if_needed(Arc::clone(&interact)).await?;

        let runtime = SchedRuntime {
            infra: self,
            plan,
            limits: self.limits.clone(),
            events,
            commands: commands_rx,
            interact,
            cancel,
            vault: Arc::new(vault),
            executor,
            native_executor,
            hash_relay,
            captures: Arc::new(crate::capture::CaptureStore::new()),
            capture_spill_root,
        };
        self.scheduler.run(runtime).await
    }

    /// Recompute plan, or verify `plan_on_disk` and use it when digest matches (or `--force`).
    pub fn resolve_plan(&self, plan_on_disk: Option<&Plan>, force: bool) -> Result<Plan> {
        let fresh = self.plan()?;
        if let Some(file) = plan_on_disk {
            // Integrity first: `file.digest` must be the hash of `file.nodes`.
            // Not subject to --force — force accepts drift vs current infra,
            // never a plan file whose contents disagree with its own digest.
            let recomputed = crate::plan::plan_digest(file);
            if recomputed != file.digest {
                return Err(CoreError::other(format!(
                    "plan file digest {} does not match its contents (recomputed {}); \
                     plan file was modified after finalize",
                    file.digest, recomputed
                )));
            }
            if file.digest != fresh.digest && !force {
                return Err(CoreError::PlanDrift {
                    file: file.digest.to_string(),
                    recomputed: fresh.digest.to_string(),
                });
            }
            Ok(file.clone())
        } else {
            Ok(fresh)
        }
    }

    pub async fn run_apply(
        &self,
        mode: RunMode,
        interact: Arc<dyn Interactor>,
        tui_events: Option<broadcast::Sender<crate::events::SchedEvent>>,
        plan: Plan,
        executor: Arc<dyn crate::exec::OpExecutor>,
        native_executor: Arc<dyn NativeExecutor>,
    ) -> Result<(RunReport, RunGuard)> {
        let run_id = RunId(Uuid::new_v4());
        let guard = RunGuard::new(&self.runtime, run_id)?;
        self.run_apply_with_guard(
            mode,
            interact,
            tui_events,
            None,
            plan,
            executor,
            native_executor,
            guard,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_apply_with_guard(
        &self,
        mode: RunMode,
        interact: Arc<dyn Interactor>,
        tui_events: Option<broadcast::Sender<crate::events::SchedEvent>>,
        commands_rx: Option<mpsc::Receiver<crate::events::SchedCommand>>,
        plan: Plan,
        executor: Arc<dyn crate::exec::OpExecutor>,
        native_executor: Arc<dyn NativeExecutor>,
        guard: RunGuard,
    ) -> Result<(RunReport, RunGuard)> {
        let vault = self.unlock_vault_session(Arc::clone(&interact)).await?;
        self.run_apply_with_guard_unlocked(
            mode,
            interact,
            tui_events,
            commands_rx,
            plan,
            executor,
            native_executor,
            guard,
            vault,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_apply_with_guard_unlocked(
        &self,
        mode: RunMode,
        interact: Arc<dyn Interactor>,
        tui_events: Option<broadcast::Sender<crate::events::SchedEvent>>,
        commands_rx: Option<mpsc::Receiver<crate::events::SchedCommand>>,
        plan: Plan,
        executor: Arc<dyn crate::exec::OpExecutor>,
        native_executor: Arc<dyn NativeExecutor>,
        guard: RunGuard,
        vault: VaultSession,
    ) -> Result<(RunReport, RunGuard)> {
        let captures = Arc::new(crate::capture::CaptureStore::new());
        self.run_apply_with_guard_captures_unlocked(
            mode,
            interact,
            tui_events,
            commands_rx,
            plan,
            executor,
            native_executor,
            guard,
            vault,
            captures,
        )
        .await
    }

    /// Like [`run_apply_with_guard`](Self::run_apply_with_guard) but uses the
    /// caller-supplied [`CaptureStore`](crate::capture::CaptureStore), so node
    /// stdout can be read back after the run (used by the MCP executor).
    #[allow(clippy::too_many_arguments)]
    pub async fn run_apply_with_guard_captures(
        &self,
        mode: RunMode,
        interact: Arc<dyn Interactor>,
        tui_events: Option<broadcast::Sender<crate::events::SchedEvent>>,
        commands_rx: Option<mpsc::Receiver<crate::events::SchedCommand>>,
        plan: Plan,
        executor: Arc<dyn crate::exec::OpExecutor>,
        native_executor: Arc<dyn NativeExecutor>,
        guard: RunGuard,
        captures: Arc<crate::capture::CaptureStore>,
    ) -> Result<(RunReport, RunGuard)> {
        let vault = self.unlock_vault_session(Arc::clone(&interact)).await?;
        self.run_apply_with_guard_captures_unlocked(
            mode,
            interact,
            tui_events,
            commands_rx,
            plan,
            executor,
            native_executor,
            guard,
            vault,
            captures,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_apply_with_guard_captures_unlocked(
        &self,
        mode: RunMode,
        interact: Arc<dyn Interactor>,
        tui_events: Option<broadcast::Sender<crate::events::SchedEvent>>,
        commands_rx: Option<mpsc::Receiver<crate::events::SchedCommand>>,
        plan: Plan,
        executor: Arc<dyn crate::exec::OpExecutor>,
        native_executor: Arc<dyn NativeExecutor>,
        guard: RunGuard,
        vault: VaultSession,
        captures: Arc<crate::capture::CaptureStore>,
    ) -> Result<(RunReport, RunGuard)> {
        let _ = mode;
        let cancel = CancellationToken::new();
        RunGuard::install_signals(cancel.clone()).await;

        let (events, _rx) = broadcast::channel(256);
        if let Some(tx) = tui_events {
            let mut sub = events.subscribe();
            tokio::spawn(async move {
                while let Ok(ev) = sub.recv().await {
                    let _ = tx.send(ev);
                }
            });
        }

        let cmd_rx = match commands_rx {
            Some(rx) => rx,
            None => {
                let (_cmd_tx, rx) = mpsc::channel(8);
                rx
            }
        };
        let spill = Some(guard.path().join("captures"));
        let runtime = SchedRuntime {
            infra: self,
            plan,
            limits: self.limits.clone(),
            events: events.clone(),
            commands: cmd_rx,
            interact,
            cancel: cancel.clone(),
            vault: Arc::new(vault),
            executor,
            native_executor,
            hash_relay: None,
            captures,
            capture_spill_root: spill,
        };
        let report = self.scheduler.run(runtime).await?;

        let report_path = guard.path().join("run-report.json");
        report.write_file(&report_path)?;

        if cancel.is_cancelled() {
            guard.teardown()?;
        }
        Ok((report, guard))
    }

    fn collect_graph(&self, report: &mut LintReport) {
        let name_by_id: FxHashMap<NodeId, &str> =
            self.nodes.iter().map(|n| (n.id, n.name.as_str())).collect();

        let mut graph = petgraph::Graph::<NodeId, ()>::new();
        let mut nodes_map = FxHashMap::default();
        for n in &self.nodes {
            let idx = graph.add_node(n.id);
            nodes_map.insert(n.id, idx);
        }
        // Report every unknown dependency, not just the first.
        for n in &self.nodes {
            let to = nodes_map[&n.id];
            for dep in &n.deps {
                match nodes_map.get(dep) {
                    Some(from) => {
                        graph.add_edge(*from, to, ());
                    }
                    None => report.error(
                        CoreError::UnknownDependency {
                            node: n.name.clone(),
                            dep: dep.to_string(),
                        },
                        "remove the dependency or add the referenced node to the infra".to_string(),
                    ),
                }
            }
        }

        // Name every cycle's members rather than a bare "cycle detected".
        report_graph_cycles(&graph, &name_by_id, "", report);
    }

    pub fn resolve_targets(&self, targets: &Targets) -> Result<Vec<MachineId>> {
        match targets {
            Targets::Machine(id) => {
                if self.machine_by_id(*id).is_none() {
                    return Err(CoreError::UnknownMachine(id.to_string()));
                }
                Ok(vec![*id])
            }
            Targets::Machines(ids) => {
                for id in ids {
                    if self.machine_by_id(*id).is_none() {
                        return Err(CoreError::UnknownMachine(id.to_string()));
                    }
                }
                Ok(ids.clone())
            }
            Targets::All => Ok(self.machines.iter().map(|m| m.id).collect()),
        }
    }

    pub fn group(&self, id: GroupId) -> Option<&Group> {
        self.groups.iter().find(|g| g.id == id)
    }

    /// Return a copy keeping only nodes whose tags match any of `tags`
    /// (`"key=value"` or bare `"key"`), plus the transitive prerequisites
    /// (`deps`) of those nodes so the DAG stays runnable. When the execution
    /// sentinel nodes are present, keep them in the filtered view and retarget
    /// `end` to the filtered leaves. Empty `tags` is a no-op (the whole infra).
    /// Machines and groups are left untouched.
    pub fn with_tag_filter(&self, tags: &[String]) -> Self {
        use std::collections::HashSet;
        if tags.is_empty() {
            return self.clone();
        }
        let by_id: HashMap<NodeId, &Node> = self.nodes.iter().map(|n| (n.id, n)).collect();
        let mut keep: HashSet<NodeId> = HashSet::new();
        let mut stack: Vec<NodeId> = Vec::new();
        for n in &self.nodes {
            if node_matches_tags(n, tags) && keep.insert(n.id) {
                stack.push(n.id);
            }
        }
        while let Some(id) = stack.pop() {
            if let Some(n) = by_id.get(&id) {
                for dep in &n.deps {
                    if keep.insert(*dep) {
                        stack.push(*dep);
                    }
                }
            }
        }
        if !keep.is_empty() {
            let mut required_machines = HashSet::new();
            for node in &self.nodes {
                if !keep.contains(&node.id) || node.body.is_graph_only() {
                    continue;
                }
                required_machines.extend(self.resolve_targets(&node.targets).unwrap_or_default());
            }
            for machine_id in required_machines {
                let connect_id = connect_node_id(machine_id);
                if by_id.contains_key(&connect_id) && keep.insert(connect_id) {
                    stack.push(connect_id);
                }
            }
            let start_id = start_node_id();
            if by_id.contains_key(&start_id) && keep.insert(start_id) {
                stack.push(start_id);
            }
            while let Some(id) = stack.pop() {
                if let Some(n) = by_id.get(&id) {
                    for dep in &n.deps {
                        if keep.insert(*dep) {
                            stack.push(*dep);
                        }
                    }
                }
            }
            let end_id = end_node_id();
            if by_id.contains_key(&end_id) {
                keep.insert(end_id);
            }
        }
        let mut filtered = self.clone();
        filtered.nodes.retain(|n| keep.contains(&n.id));
        retarget_end_to_filtered_leaves(&mut filtered.nodes);
        filtered
    }

    /// Build a read-only [`GraphView`](crate::graph::GraphView) of the planning
    /// DAG: every node with its resolved target machines, tags, and dependency
    /// edges. No fact gathering or vault access. Filter via
    /// [`GraphView::select`](crate::graph::GraphView::select).
    pub fn graph_view(&self) -> Result<crate::graph::GraphView> {
        use crate::graph::{GraphEdge, GraphNode, GraphView};
        use std::collections::HashSet;

        let mut nodes = Vec::with_capacity(self.nodes.len());
        for n in &self.nodes {
            let machines = self
                .resolve_targets(&n.targets)?
                .iter()
                .filter_map(|id| self.machine_by_id(*id))
                .map(|m| m.name.clone())
                .collect();
            nodes.push(GraphNode {
                id: n.id.0.to_string(),
                name: n.name.clone(),
                description: n.description.clone(),
                kind: n.body.kind_label().to_string(),
                machines,
                tags: n
                    .tags
                    .iter()
                    .map(|t| format!("{}={}", t.key, t.value))
                    .collect(),
                deps: n.deps.iter().map(|d| d.0.to_string()).collect(),
            });
        }

        let ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        let mut edges = Vec::new();
        for n in &nodes {
            for dep in &n.deps {
                if ids.contains(dep.as_str()) {
                    edges.push(GraphEdge {
                        from: dep.clone(),
                        to: n.id.clone(),
                    });
                }
            }
        }
        Ok(GraphView { nodes, edges })
    }
}

/// True if `node` carries a tag matching any needle (`"key=value"`, key, or value).
fn node_matches_tags(node: &Node, needles: &[String]) -> bool {
    needles.iter().any(|needle| {
        node.tags.iter().any(|t| {
            needle == &format!("{}={}", t.key, t.value)
                || needle.as_str() == t.key
                || needle.as_str() == t.value
        })
    })
}

fn retarget_end_to_filtered_leaves(nodes: &mut [Node]) {
    let end_id = end_node_id();
    if !nodes.iter().any(|n| n.id == end_id) {
        return;
    }

    let ids: std::collections::HashSet<NodeId> = nodes.iter().map(|n| n.id).collect();
    let depended_on: std::collections::HashSet<NodeId> = nodes
        .iter()
        .filter(|n| n.id != end_id)
        .flat_map(|n| n.deps.iter().copied())
        .filter(|id| ids.contains(id))
        .collect();
    let mut deps: Vec<NodeId> = nodes
        .iter()
        .filter(|n| n.id != end_id)
        .filter(|n| !n.is_lazy())
        .filter(|n| !(n.body.is_group_bookend() || n.body.is_connect()))
        .filter(|n| !depended_on.contains(&n.id))
        .map(|n| n.id)
        .collect();
    if deps.is_empty() {
        deps = nodes
            .iter()
            .filter(|n| n.id != end_id)
            .filter(|n| !matches!(n.policy.run_policy, RunPolicy::Lazy))
            .filter(|n| !depended_on.contains(&n.id))
            .map(|n| n.id)
            .collect();
    }

    if let Some(end) = nodes.iter_mut().find(|n| n.id == end_id) {
        end.deps = deps;
    }
}

fn has_non_lazy_dependent(
    start: NodeId,
    policy_by_id: &FxHashMap<NodeId, RunPolicy>,
    dependents_by_id: &FxHashMap<NodeId, Vec<NodeId>>,
) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut stack = dependents_by_id.get(&start).cloned().unwrap_or_default();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        if !matches!(policy_by_id.get(&id), Some(RunPolicy::Lazy)) {
            return true;
        }
        if let Some(next) = dependents_by_id.get(&id) {
            stack.extend(next.iter().copied());
        }
    }
    false
}

/// Builder helpers used by infrazeug-api.
pub fn remote_machine(id: MachineId, name: impl Into<String>, ssh: SshConfig) -> Machine {
    Machine {
        id,
        name: name.into(),
        kind: MachineKind::Remote { ssh, os: None },
        vars: VarSet::new(),
        groups: Vec::new(),
        tags: Vec::new(),
        max_parallel_nodes: None,
        lifecycle: crate::machine::Lifecycle::Persistent,
        like: None,
        lazy: false,
    }
}

impl Infra {
    pub async fn unlock_vault_session(
        &self,
        interact: Arc<dyn Interactor>,
    ) -> Result<VaultSession> {
        let mut vault = self.build_vault_session();
        vault.unlock_if_needed(interact).await?;
        Ok(vault)
    }

    fn build_vault_session(&self) -> VaultSession {
        let Some(store_path) = &self.runtime.vault_store else {
            return VaultSession::default();
        };
        let backend = Arc::new(FsBackend::new(store_path));
        let store = VaultStore::new(backend, store_path.clone());
        let pending = self.vault_data_keys.clone();
        VaultSession::from_store(store, pending)
    }
}

/// Report every dependency cycle in `graph` via Tarjan strongly-connected
/// components (the "reduce to a DAG" check). Each SCC larger than one node — or a
/// single node with a self-edge — is an unbreakable dependency loop; its members
/// are named so the playbook author can see the loop. `context`, when non-empty,
/// prefixes the message (e.g. a dynamic group's template).
fn report_graph_cycles(
    graph: &petgraph::Graph<NodeId, ()>,
    name_by_id: &FxHashMap<NodeId, &str>,
    context: &str,
    report: &mut LintReport,
) {
    for scc in petgraph::algo::tarjan_scc(graph) {
        let is_cycle = scc.len() > 1
            || scc
                .first()
                .is_some_and(|&idx| graph.find_edge(idx, idx).is_some());
        if !is_cycle {
            continue;
        }
        let members: Vec<&str> = scc
            .iter()
            .map(|&idx| name_by_id.get(&graph[idx]).copied().unwrap_or("?"))
            .collect();
        let chain = members.join(" -> ");
        let detail = if context.is_empty() {
            chain
        } else {
            format!("{context}: {chain}")
        };
        report.error(
            CoreError::Cycle(detail),
            "break the dependency cycle so the nodes form a DAG".to_string(),
        );
    }
}

pub fn local_machine(id: MachineId, name: impl Into<String>) -> Machine {
    Machine {
        id,
        name: name.into(),
        kind: MachineKind::Local,
        vars: VarSet::new(),
        groups: Vec::new(),
        tags: Vec::new(),
        max_parallel_nodes: None,
        lifecycle: crate::machine::Lifecycle::Persistent,
        like: None,
        lazy: false,
    }
}

pub fn shell_node(id: NodeId, name: impl Into<String>, op: ShellOp, targets: Targets) -> Node {
    NodeBuilder::shell(id, op, targets).name(name).build()
}

pub fn barrier_node(
    id: NodeId,
    name: impl Into<String>,
    targets: Targets,
    deps: Vec<NodeId>,
) -> Node {
    NodeBuilder::barrier(id, targets)
        .name(name)
        .deps(deps)
        .build()
}

pub fn begin_node(
    id: NodeId,
    name: impl Into<String>,
    targets: Targets,
    deps: Vec<NodeId>,
) -> Node {
    NodeBuilder::begin(id, targets)
        .name(name)
        .deps(deps)
        .build()
}

pub fn finish_node(
    id: NodeId,
    name: impl Into<String>,
    targets: Targets,
    deps: Vec<NodeId>,
) -> Node {
    NodeBuilder::finish(id, targets)
        .name(name)
        .deps(deps)
        .build()
}

/// Deterministic id for the execution graph's global start node.
pub fn start_node_id() -> NodeId {
    let seed = "infrazeug/start";
    NodeId(uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        seed.as_bytes(),
    ))
}

pub fn start_node_on(machine: MachineId) -> Node {
    NodeBuilder::begin(start_node_id(), Targets::Machine(machine))
        .name("start")
        .build()
}

pub fn start_node() -> Node {
    NodeBuilder::begin(start_node_id(), Targets::All)
        .name("start")
        .build()
}

/// Deterministic id for the execution graph's global end node.
pub fn end_node_id() -> NodeId {
    let seed = "infrazeug/end";
    NodeId(uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        seed.as_bytes(),
    ))
}

pub fn end_node_on(machine: MachineId, deps: Vec<NodeId>) -> Node {
    NodeBuilder::finish(end_node_id(), Targets::Machine(machine))
        .name("end")
        .deps(deps)
        .build()
}

pub fn end_node(deps: Vec<NodeId>) -> Node {
    NodeBuilder::finish(end_node_id(), Targets::All)
        .name("end")
        .deps(deps)
        .build()
}

/// Deterministic id for a machine's connectivity / agent-upload head node.
///
/// Stable across runs so the connect node has a fixed identity for replay and
/// for downstream dependency wiring.
pub fn connect_node_id(machine_id: MachineId) -> NodeId {
    let seed = format!("infrazeug/connect/{}", machine_id.0);
    NodeId(uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_OID,
        seed.as_bytes(),
    ))
}

pub fn connect_node(
    id: NodeId,
    name: impl Into<String>,
    targets: Targets,
    deps: Vec<NodeId>,
) -> Node {
    NodeBuilder::connect(id, targets)
        .name(name)
        .deps(deps)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;
    use crate::id::{MachineId, NodeId};
    use crate::transport::TransportChoice;
    use uuid::Uuid;

    #[test]
    fn tag_filter_keeps_matches_and_their_deps() {
        use crate::id::Tag;
        let m = MachineId(Uuid::new_v4());
        let base = NodeId(Uuid::new_v4());
        let web = NodeId(Uuid::new_v4());
        let op = || ShellOp::run(infrazeug_shell::argv!["true"]);

        let base_node = shell_node(base, "base", op(), Targets::Machine(m));
        let mut web_node = shell_node(web, "web", op(), Targets::Machine(m));
        web_node.deps.push(base);
        web_node.tags.push(Tag::new("app", "web"));

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(base_node)
            .unwrap()
            .add_node(web_node)
            .unwrap();

        // app=web matches `web`; its dep `base` is pulled in.
        let filtered = infra.with_tag_filter(&["app=web".to_string()]);
        let names: std::collections::HashSet<&str> =
            filtered.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["base", "web"].into_iter().collect());

        // Bare key matches too.
        assert_eq!(infra.with_tag_filter(&["app".to_string()]).nodes.len(), 2);
        // Bare value matches too.
        assert_eq!(infra.with_tag_filter(&["web".to_string()]).nodes.len(), 2);
        // No match drops everything.
        assert!(infra
            .with_tag_filter(&["none".to_string()])
            .nodes
            .is_empty());
        // Empty filter is a no-op.
        assert_eq!(infra.with_tag_filter(&[]).nodes.len(), 2);
    }

    #[test]
    fn tag_filter_keeps_execution_sentinels_and_required_connect_nodes() {
        use crate::id::Tag;
        let web_machine = MachineId(Uuid::new_v4());
        let db_machine = MachineId(Uuid::new_v4());
        let web_base = NodeId(Uuid::new_v4());
        let web = NodeId(Uuid::new_v4());
        let db = NodeId(Uuid::new_v4());
        let web_connect = connect_node_id(web_machine);
        let db_connect = connect_node_id(db_machine);
        let op = || ShellOp::run(infrazeug_shell::argv!["true"]);

        let mut web_base_node =
            shell_node(web_base, "base/web", op(), Targets::Machine(web_machine));
        web_base_node.deps.push(web_connect);
        let mut web_node = shell_node(web, "web", op(), Targets::Machine(web_machine));
        web_node.deps.push(web_base);
        web_node.tags.push(Tag::new("app", "web"));
        let mut db_node = shell_node(db, "db", op(), Targets::Machine(db_machine));
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
            .unwrap();

        let filtered = infra.with_tag_filter(&["web".to_string()]);
        let names: std::collections::HashSet<&str> =
            filtered.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names,
            ["start", "connect/web-host", "base/web", "web", "end"]
                .into_iter()
                .collect()
        );

        let end = filtered
            .nodes
            .iter()
            .find(|n| n.id == end_node_id())
            .expect("end node");
        assert_eq!(end.deps, vec![web]);
        filtered.plan().expect("filtered infra should plan");
    }

    #[test]
    fn duplicate_machine_name_rejected() {
        let id1 = MachineId(Uuid::new_v4());
        let id2 = MachineId(Uuid::new_v4());
        let result = Infra::new()
            .add_machine(local_machine(id1, "dup"))
            .unwrap()
            .add_machine(local_machine(id2, "dup"));
        assert!(matches!(
            result,
            Err(CoreError::DuplicateName {
                kind: "machine",
                ..
            })
        ));
    }

    #[test]
    fn lint_report_collects_all_unknown_deps() {
        let m = MachineId(Uuid::new_v4());
        let n1 = NodeId(Uuid::new_v4());
        let mut node = shell_node(
            n1,
            "two-bad-deps",
            ShellOp::Run {
                argv: vec!["true".into()],
                cwd: None,
                env: Vec::new(),
            },
            Targets::Machine(m),
        );
        node.deps = vec![NodeId(Uuid::new_v4()), NodeId(Uuid::new_v4())];
        let report = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(node)
            .unwrap()
            .lint_report();
        // Both missing deps are reported in a single pass, not fail-fast.
        assert_eq!(
            report
                .errors()
                .filter(|d| d.code() == "unknown-dependency")
                .count(),
            2
        );
        // Diagnostics serialize to a stable machine-readable shape.
        let json = report.to_json();
        assert_eq!(json["diagnostics"][0]["severity"], "error");
        assert_eq!(json["diagnostics"][0]["code"], "unknown-dependency");
        assert!(json["diagnostics"][0]["help"].is_string());
    }

    #[test]
    fn cycle_in_deps_rejected() {
        let m = MachineId(Uuid::new_v4());
        let n1 = NodeId(Uuid::new_v4());
        let n2 = NodeId(Uuid::new_v4());
        let mut a = shell_node(
            n1,
            "a",
            ShellOp::Run {
                argv: vec!["true".into()],
                cwd: None,
                env: Vec::new(),
            },
            Targets::Machine(m),
        );
        let mut b = shell_node(
            n2,
            "b",
            ShellOp::Run {
                argv: vec!["true".into()],
                cwd: None,
                env: Vec::new(),
            },
            Targets::Machine(m),
        );
        a.deps = vec![n2];
        b.deps = vec![n1];
        let report = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(a)
            .unwrap()
            .add_node(b)
            .unwrap()
            .lint_report();
        let cycle = report
            .errors()
            .find(|d| d.code() == "cycle")
            .expect("cycle diagnostic");
        // Both nodes are named in the cycle message.
        assert!(cycle.message().contains("a"));
        assert!(cycle.message().contains("b"));
        assert!(matches!(
            Infra::new()
                .add_machine(local_machine(m, "local"))
                .unwrap()
                .lint(),
            Ok(())
        ));
    }

    #[test]
    fn lazy_node_without_non_lazy_dependent_warns() {
        let m = MachineId(Uuid::new_v4());
        let lazy = NodeId(Uuid::new_v4());
        let mut node = shell_node(
            lazy,
            "build-cache",
            ShellOp::run(vec!["true".into()]),
            Targets::Machine(m),
        );
        node.policy.run_policy = RunPolicy::Lazy;

        let report = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(node)
            .unwrap()
            .lint_report();

        let warning = report
            .warnings()
            .find(|d| d.code() == "lazy-node-undemandable")
            .expect("lazy warning");
        assert!(warning.message().contains("build-cache"));
    }

    #[test]
    fn lazy_chain_with_non_lazy_consumer_does_not_warn() {
        let m = MachineId(Uuid::new_v4());
        let lazy_a = NodeId(Uuid::new_v4());
        let lazy_b = NodeId(Uuid::new_v4());
        let consumer = NodeId(Uuid::new_v4());
        let mut a = shell_node(
            lazy_a,
            "a",
            ShellOp::run(vec!["true".into()]),
            Targets::Machine(m),
        );
        a.policy.run_policy = RunPolicy::Lazy;
        let mut b = shell_node(
            lazy_b,
            "b",
            ShellOp::run(vec!["true".into()]),
            Targets::Machine(m),
        );
        b.policy.run_policy = RunPolicy::Lazy;
        b.deps = vec![lazy_a];
        let mut c = shell_node(
            consumer,
            "c",
            ShellOp::run(vec!["true".into()]),
            Targets::Machine(m),
        );
        c.deps = vec![lazy_b];

        let report = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(a)
            .unwrap()
            .add_node(b)
            .unwrap()
            .add_node(c)
            .unwrap()
            .lint_report();

        assert_eq!(
            report
                .warnings()
                .filter(|d| d.code() == "lazy-node-undemandable")
                .count(),
            0
        );
    }

    #[test]
    fn dynamic_template_partial_cycle_rejected() {
        use crate::dynamic::{template_placeholder_machine, DynamicGroup};
        use crate::node::{FailPolicy, NodeBuilder};

        let m = MachineId(Uuid::new_v4());
        let disc = NodeId(Uuid::new_v4());
        let (head, b, c) = (
            NodeId(Uuid::new_v4()),
            NodeId(Uuid::new_v4()),
            NodeId(Uuid::new_v4()),
        );

        // head -> b -> c -> b : `head` is a valid head node, but b<->c is a cycle the
        // head-node check alone cannot catch.
        let placeholder = Targets::Machine(template_placeholder_machine());
        let op = || ShellOp::run(vec!["true".into()]);
        let template = vec![
            NodeBuilder::shell(head, op(), placeholder.clone())
                .name("head")
                .build(),
            NodeBuilder::shell(b, op(), placeholder.clone())
                .name("b")
                .deps(vec![head, c])
                .build(),
            NodeBuilder::shell(c, op(), placeholder.clone())
                .name("c")
                .deps(vec![b])
                .build(),
        ];

        let mut infra = Infra::new()
            .add_machine(local_machine(m, "controller"))
            .unwrap()
            .add_node(
                NodeBuilder::native_with_input(
                    disc,
                    "test.discover",
                    serde_cbor::Value::Null,
                    Targets::Machine(m),
                )
                .name("discover")
                .build(),
            )
            .unwrap();
        infra.push_dynamic_group(DynamicGroup {
            label: "workers".into(),
            discovery_node: disc,
            template,
            template_entry_deps: vec![disc],
            fail_policy: FailPolicy::Tolerate {
                max_failed: usize::MAX,
            },
            max_parallel_machines: None,
        });

        let report = infra.lint_report();
        let cycle = report
            .errors()
            .find(|d| d.code() == "cycle")
            .expect("template cycle diagnostic");
        assert!(cycle.message().contains("dynamic group `workers` template"));
        assert!(cycle.message().contains('b'));
        assert!(cycle.message().contains('c'));
        // The valid head node is not part of the reported cycle.
        assert!(!cycle.message().contains("head"));
    }

    #[test]
    fn unknown_dependency_rejected() {
        let m = MachineId(Uuid::new_v4());
        let n1 = NodeId(Uuid::new_v4());
        let mut node = shell_node(
            n1,
            "orphan-dep",
            ShellOp::Run {
                argv: vec!["true".into()],
                cwd: None,
                env: Vec::new(),
            },
            Targets::Machine(m),
        );
        node.deps = vec![NodeId(Uuid::new_v4())];
        let err = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(node)
            .unwrap()
            .lint()
            .unwrap_err();
        // Aggregated into a Lint error carrying a typed unknown-dependency diagnostic.
        let CoreError::Lint(report) = err else {
            panic!("expected lint error, got {err:?}");
        };
        assert!(report.errors().any(|d| d.code() == "unknown-dependency"));
    }

    #[test]
    fn transport_for_machine_respects_override() {
        let m = MachineId(Uuid::new_v4());
        let infra = Infra::new()
            .add_machine(remote_machine(m, "remote", SshConfig::new("root@example")))
            .unwrap()
            .with_transport_choice(m, TransportChoice::SshAgentPush)
            .with_default_remote_transport(TransportChoice::SshAgentless);
        let machine = infra.machine_by_id(m).unwrap();
        assert_eq!(
            infra.transport_for_machine(machine),
            TransportChoice::SshAgentPush
        );
    }

    #[test]
    fn plan_digest_changes_when_node_body_changes() {
        let m = MachineId(Uuid::new_v4());
        let n = NodeId(Uuid::new_v4());
        let infra_a = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(shell_node(
                n,
                "same-node",
                ShellOp::Run {
                    argv: vec!["echo".into(), "a".into()],
                    cwd: None,
                    env: Vec::new(),
                },
                Targets::Machine(m),
            ))
            .unwrap();
        let infra_b = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(shell_node(
                n,
                "same-node",
                ShellOp::Run {
                    argv: vec!["echo".into(), "b".into()],
                    cwd: None,
                    env: Vec::new(),
                },
                Targets::Machine(m),
            ))
            .unwrap();

        let plan_a = infra_a.plan().unwrap();
        let plan_b = infra_b.plan().unwrap();

        assert_ne!(plan_a.digest, plan_b.digest);
        assert_ne!(plan_a.nodes[0].fingerprint, plan_b.nodes[0].fingerprint);
    }

    #[tokio::test]
    async fn preview_reports_real_native_outcomes() {
        use crate::native_exec::LocalNativeExecutor;
        use async_trait::async_trait;
        use infrazeug_native::{
            encode_input, MethodRegistry, NativeResult, NodeCtx, NodeMethod, PlanCtx,
            PlanMethodOutcome, Result as NativeResultT,
        };
        use serde::{Deserialize, Serialize};

        // Fake method whose `plan` simulates an `observe`: present → Unchanged,
        // absent → Changed (create). No network.
        #[derive(Default, Clone, Serialize, Deserialize)]
        struct Spec {
            exists: bool,
        }
        struct Fake;
        #[async_trait]
        impl NodeMethod for Fake {
            type Input = Spec;
            type Output = ();
            fn name(&self) -> &'static str {
                "test.fake_preview"
            }
            fn idempotent(&self) -> bool {
                true
            }
            async fn plan(&self, _ctx: &PlanCtx, spec: &Spec) -> NativeResultT<PlanMethodOutcome> {
                Ok(if spec.exists {
                    PlanMethodOutcome::Unchanged
                } else {
                    PlanMethodOutcome::Changed
                })
            }
            async fn execute(&self, _ctx: &NodeCtx, _spec: Spec) -> NativeResultT<NativeResult> {
                Ok(NativeResult::unchanged("noop"))
            }
        }

        let m = MachineId(Uuid::new_v4());
        let absent = NodeId(Uuid::new_v4());
        let present = NodeId(Uuid::new_v4());
        let input = |exists: bool| encode_input(&Spec { exists }).unwrap();

        let infra = Infra::new()
            .add_machine(local_machine(m, "local"))
            .unwrap()
            .add_node(
                NodeBuilder::native_with_input(
                    absent,
                    "test.fake_preview",
                    input(false),
                    Targets::Machine(m),
                )
                .name("absent")
                .build(),
            )
            .unwrap()
            .add_node(
                NodeBuilder::native_with_input(
                    present,
                    "test.fake_preview",
                    input(true),
                    Targets::Machine(m),
                )
                .name("present")
                .build(),
            )
            .unwrap();

        let mut reg = MethodRegistry::new();
        reg.register(Fake);
        let exec = LocalNativeExecutor::new(Arc::new(reg));

        let preview = infra.preview(&exec).await.unwrap();
        let counts = preview.counts();
        assert_eq!(counts.change, 1);
        assert_eq!(counts.in_sync, 1);

        let node = |id: NodeId| preview.nodes.iter().find(|n| n.node_id == id).unwrap();
        assert_eq!(node(absent).outcome, PlanOutcome::Changed);
        assert!(node(absent).previewable);
        assert_eq!(node(present).outcome, PlanOutcome::Unchanged);
    }
}
