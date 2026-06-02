//! User-facing builder API for infrazeug playbooks (SOUL §11 step 3).
//!
//! Playbook binaries depend on this crate (often via a single `infrazeug` meta
//! dependency) to construct an [`Infra`], run `plan` / `apply` / `test`, wire
//! emulation and agent builds, and optionally expose MCP tools.
//!
//! # Layout
//!
//! - **Re-exports** — [`Infra`], [`ShellOp`], [`template!`](infrazeug_templates::template),
//!   [`vars`], and MCP types so playbooks need few crate lines.
//! - [`cli`] — canonical `clap` surface shared by examples, playbooks, and
//!   [`infrazeug-cli`]; call [`run`] from `main` with a playbook registry closure.
//! - [`RunPrepare`] / [`infra_for_run`] — emulation prep, agent cross-build, transport connect.
//! - [`mcp_serve`] — [`McpExt::mcp`] closes the loop: tools build an [`Infra`] and
//!   [`ApiExecutor`] runs the real scheduler path.
//! - [`pull_cli`] — pull-mode subcommands (`machine`, `plan-op`, `serve-pull`, `bootstrap`).
//!
//! # Minimal playbook
//!
//! ```ignore
//! use infrazeug_api::{cli, default_infra, Infra, RunCommands, RunConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     cli::init_tracing();
//!     cli::run(
//!         std::env::args(),
//!         RunConfig::new(env!("CARGO_PKG_NAME")).commands(RunCommands::playbook()),
//!         |_| Ok(default_infra()),
//!     )
//!     .await
//! }
//! ```
//!
//! [`Infra`]: infrazeug_core::Infra
//! [`ShellOp`]: infrazeug_shell::ShellOp
//! [`run`]: cli::run
//! [`RunPrepare`]: emulation::RunPrepare
//! [`infra_for_run`]: emulation::infra_for_run
//! [`McpExt::mcp`]: mcp_serve::McpExt::mcp
//! [`ApiExecutor`]: mcp_serve::ApiExecutor

pub mod cli;
pub mod mcp_cli;
pub mod mcp_serve;
pub mod playbooks;
pub mod prepare;
pub mod probe;
pub mod pull_cli;
pub mod report_emit;
pub mod transport_env;

pub mod dynamic;
mod emulation;
mod native_builder;
mod ssh_auth;

pub use cli::{
    dispatch, init_tracing, parse_playbook_flag, run, ApplyParsed, ExtraSubcommand,
    PlaybookCommand, PlaybookCommands, RunBuildContext, RunCommands, RunConfig, RunContext,
    TestParsed, PLAYBOOK_FLAG, PLAYBOOK_SUBCOMMANDS,
};
pub use infrazeug_mcp::{self, CaptureOut, InfraExecutor, McpBuilder, McpCtx, ToolRun};
pub use mcp_cli::{dispatch_mcp_serve, McpServeMode, MCP_NESTED_SUBCOMMANDS, MCP_SUBCOMMANDS};
pub use mcp_serve::{ApiExecutor, McpExt};
pub use playbooks::{build_from_registry, PlaybookEntry, PlaybookRegistry};
pub use probe::{export_probe_targets, ProbeExport, RemoteProbeTarget, PROBE_SUBCOMMAND};
pub use pull_cli::{
    dispatch_pull, BootstrapExec, MachineCmd, PlanCmd, PullCommand, PullCommandSet, PullCommands,
    PullContext, PULL_SUBCOMMANDS,
};
pub use report_emit::{debug_requested, print_run_report, report_has_failures, terminal_ui_active};
pub use transport_env::{default_remote_transport, parse_transport_name, transport_name};

pub use emulation::{infra_for_run, setup_emulation, teardown_containers, RunPrepare};
pub use infrazeug_core::{self, vars};
pub use infrazeug_core::{
    begin_node_id, finish_node_id, AddressFamily, AsyncNodeGroup, Group, GroupId, Infra, Machine,
    MachineId, MachineKind, Node, NodeBuilder, NodeId, OutputChangePolicy, OutputChangeRule,
    OutputMatchStatus, OutputMatchStream, Plan, RunPolicy, RunReport, RuntimeConfig, SshConfig,
    SyncNodeGroup, Targets, TestReport, TransportChoice, VarSet,
};
pub use infrazeug_core::{infra, uuid};
pub use infrazeug_native::{
    builtin_registry, encode_input, EchoInput, EchoMethod, NativeResult, NativeStatus, NodeMethod,
    PingInput, PingMethod, NATIVE_ECHO, NATIVE_PING,
};
pub use infrazeug_shell::{self, argv, EnvVarSource, FileSource, ShellOp, SyncDirOptions};
pub use infrazeug_templates::{self, escape, template};

use infrazeug_core::exec::OpExecutor;
use infrazeug_core::infra::{barrier_node, local_machine, remote_machine, shell_node};
use infrazeug_core::interactor::{
    AutoDenyInteractor, Interactor, LineInteractor, NoPromptInteractor,
};
use infrazeug_core::native_exec::{LocalNativeExecutor, NativeExecutor, RoutingNativeExecutor};
use infrazeug_core::runtime::RunMode;
use infrazeug_core::scheduler::DefaultScheduler;
use infrazeug_core::PlanOutcome;
use infrazeug_core::VaultSession;
use infrazeug_native::MethodRegistry;
use infrazeug_transport::TransportFactory;
use infrazeug_tui::TuiInteractor;
use std::path::PathBuf;
use std::sync::Arc;

/// Infra graph plus tier-1 native methods registered at playbook build time.
#[derive(Clone)]
pub struct PlaybookBundle {
    pub infra: Infra,
    pub methods: MethodRegistry,
}

impl PlaybookBundle {
    pub fn new(infra: Infra) -> Self {
        Self {
            infra,
            methods: MethodRegistry::new(),
        }
    }

    pub fn from_infra(infra: Infra) -> Self {
        Self::new(infra)
    }

    pub fn merged_methods(&self) -> MethodRegistry {
        let mut merged = builtin_registry();
        merged.merge(self.methods.clone());
        merged
    }

    pub fn plan(&self) -> infrazeug_core::Result<Plan> {
        self.infra.plan_with_methods(Some(&self.merged_methods()))
    }

    pub fn lint(&self) -> infrazeug_core::Result<()> {
        self.infra.lint_with_methods(&self.merged_methods())
    }

    pub fn lint_report(&self) -> infrazeug_core::LintReport {
        self.infra
            .lint_report_with_methods(Some(&self.merged_methods()))
    }

    pub fn with_tag_filter(&self, tags: &[String]) -> Self {
        Self {
            infra: self.infra.with_tag_filter(tags),
            methods: self.methods.clone(),
        }
    }

    pub fn with_default_remote_transport(self, choice: TransportChoice) -> Self {
        Self {
            infra: self.infra.with_default_remote_transport(choice),
            methods: self.methods,
        }
    }

    pub fn with_runtime(self, runtime: RuntimeConfig) -> Self {
        Self {
            infra: self.infra.with_runtime(runtime),
            methods: self.methods,
        }
    }

    pub fn with_transport_choice(self, machine_id: MachineId, choice: TransportChoice) -> Self {
        Self {
            infra: self.infra.with_transport_choice(machine_id, choice),
            methods: self.methods,
        }
    }
}

fn transport_as_native(factory: Arc<TransportFactory>) -> Arc<dyn NativeExecutor> {
    factory
}

fn build_native_executor(
    infra: &Infra,
    methods: MethodRegistry,
    transport: Arc<dyn NativeExecutor>,
) -> Arc<dyn NativeExecutor> {
    let mut merged = builtin_registry();
    merged.merge(methods);
    RoutingNativeExecutor::new(Arc::new(infra.clone()), Arc::new(merged), transport)
}

/// In-process executor (controller registry only) used for read-only previews —
/// no transport connection needed, since only `Local` native nodes are inspected.
fn preview_native_executor(bundle: &PlaybookBundle) -> Arc<dyn NativeExecutor> {
    let mut merged = builtin_registry();
    merged.merge(bundle.methods.clone());
    Arc::new(LocalNativeExecutor::new(Arc::new(merged)))
}

/// Run the read-only preview and print a `change / in-sync / unknown` summary.
///
/// Issues read-only API calls (`observe`) for previewable (`Local` native) nodes;
/// applies nothing.
async fn print_dry_run_preview(bundle: &PlaybookBundle) -> anyhow::Result<()> {
    let executor = preview_native_executor(bundle);
    let preview = bundle.infra.preview(executor.as_ref()).await?;
    let counts = preview.counts();
    println!(
        "Plan: {} to change, {} in sync, {} unknown (read-only preview, applies nothing)",
        counts.change, counts.in_sync, counts.unknown
    );
    for node in &preview.nodes {
        let (mark, label) = match node.outcome {
            PlanOutcome::Changed => ("+", "change"),
            PlanOutcome::Unchanged => ("=", "in sync"),
            PlanOutcome::Unknown => ("?", "unknown"),
        };
        match &node.note {
            Some(note) => println!("  {mark} {label:<8} {} ({note})", node.name),
            None => println!("  {mark} {label:<8} {}", node.name),
        }
    }
    Ok(())
}

#[derive(Default)]
pub struct ApplyOptions {
    pub tui: bool,
    pub watch: bool,
    pub dry_run: bool,
    pub force: bool,
    pub plan_path: Option<PathBuf>,
    pub emulate_first: bool,
    pub unpinned: bool,
    pub debug: bool,
}

#[derive(Default)]
pub struct TestOptions {
    pub tui: bool,
    pub watch: bool,
    pub dry_run: bool,
    pub force: bool,
    pub plan_path: Option<PathBuf>,
    pub unpinned: bool,
    pub debug: bool,
}

/// Emulation, agent builds, and per-machine transport connect (before scheduler/TUI).
struct PreparedRun {
    prepared: RunPrepare,
    guard: infrazeug_core::RunGuard,
    factory: Arc<TransportFactory>,
    test_report: TestReport,
}

struct ApplyStartup {
    prep: PreparedRun,
    vault: VaultSession,
}

async fn prepare_run_transports(
    infra: &Infra,
    mode: RunMode,
    events: Option<tokio::sync::broadcast::Sender<infrazeug_core::SchedEvent>>,
    interact: Option<Arc<dyn Interactor>>,
    secrets: Option<Arc<dyn infrazeug_native::SecretSource>>,
) -> anyhow::Result<PreparedRun> {
    use infrazeug_core::events::{MachinePreparePhase, SchedEvent};

    let emit = |ev: SchedEvent| {
        if let Some(tx) = &events {
            let _ = tx.send(ev);
        }
    };

    let machine_summaries: Vec<_> = infra.machines.iter().map(|m| (m.id, m.summary())).collect();
    emit(SchedEvent::PrepareStarted {
        machine_summaries: machine_summaries.clone(),
    });
    for (id, _) in &machine_summaries {
        emit(SchedEvent::PrepareMachine {
            machine: *id,
            phase: MachinePreparePhase::Pending,
            detail: None,
        });
    }

    let run_id = infrazeug_core::id::RunId(uuid::Uuid::new_v4());
    let guard = infrazeug_core::RunGuard::new(&infra.runtime, run_id)?;
    let mut prepared = infra_for_run(infra, mode, guard.run_id);
    let test_report = prepared.test_report.clone();
    let agent_workspace = infrazeug_build::infrazeug_workspace_root();
    let release = std::env::var("INFRAZEUG_RELEASE")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let factory =
        TransportFactory::new(guard.path().to_path_buf(), agent_workspace.clone(), release);
    let factory_for_prepare = Arc::clone(&factory);
    factory.set_prepare_events(events.clone()).await;

    let result: anyhow::Result<PreparedRun> = async {
        emit(SchedEvent::PrepareGlobal {
            message: "emulation / containers".into(),
        });
        setup_emulation(&mut prepared, &guard, &factory_for_prepare)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        // Wire the interactive SSH auth resolver (prompt / vault). It serves the
        // first connection and arch probe of every machine — including lazy and
        // dynamically-discovered ones that connect later in the run.
        let resolver = Arc::new(ssh_auth::ApiSshAuthResolver::new(
            interact.clone(),
            secrets.clone(),
            guard.path(),
        ));
        factory_for_prepare
            .set_ssh_resolver(Some(resolver.clone()))
            .await;
        // Prompt up front (in declaration order) for statically-declared
        // machines, so operators are not asked mid-apply; lazy / discovered
        // machines resolve lazily through the same cached resolver.
        prewarm_ssh_auth(&prepared.infra, resolver.as_ref()).await?;
        // Agent builds are now on-demand per triple inside the transport factory
        // (probe + cross-build + upload happen when a machine's Connect node or
        // first node runs), so there is no eager pre-apply agent build phase.
        emit(SchedEvent::PrepareGlobal {
            message: "connect transports".into(),
        });
        factory_for_prepare
            .prepare(&prepared.infra)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(PreparedRun {
            prepared,
            guard,
            factory: factory_for_prepare,
            test_report,
        })
    }
    .await;

    // Keep the event sink wired for the whole run: lazy machines, discovered
    // (dynamic-group) machines and reconnects build their backend mid-run, and
    // the metrics forwarder for a push agent is only created when the factory
    // has a sender at connect time (`agent_metrics_sink`). Clearing it here
    // silently dropped cpu/mem metrics for every post-prepare connect.
    match &result {
        Ok(_) => emit(SchedEvent::PrepareFinished {
            ok: true,
            message: None,
        }),
        Err(e) => emit(SchedEvent::PrepareFinished {
            ok: false,
            message: Some(e.to_string()),
        }),
    }
    result
}

/// Resolve interactive SSH secrets up front for every statically-declared,
/// non-lazy remote machine, in declaration order, so the operator is prompted
/// before the apply rather than mid-run. The resolver caches each result, so the
/// subsequent connect (and any lazy / discovered machine) reuses it. Returns the
/// first resolution error (e.g. a prompt with no interactor, or a cancel).
async fn prewarm_ssh_auth(
    infra: &Infra,
    resolver: &ssh_auth::ApiSshAuthResolver,
) -> anyhow::Result<()> {
    use infrazeug_transport::SshAuthResolver;

    for m in &infra.machines {
        if m.lazy {
            continue;
        }
        let MachineKind::Remote { ssh, .. } = &m.kind else {
            continue;
        };
        if ssh.auth.is_interactive() {
            resolver
                .askpass_file(m.id, ssh)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        }
    }
    Ok(())
}

async fn prepare_apply_startup(
    infra: &Infra,
    mode: RunMode,
    interact: Arc<dyn Interactor>,
    events: Option<tokio::sync::broadcast::Sender<infrazeug_core::SchedEvent>>,
    unlock_ready: Option<tokio::sync::watch::Sender<bool>>,
) -> anyhow::Result<ApplyStartup> {
    use infrazeug_core::events::SchedEvent;

    let emit = |ev: SchedEvent| {
        if let Some(tx) = &events {
            let _ = tx.send(ev);
        }
    };

    // Unlock before transport bootstrap so the TUI is not left at "6/6 ready"
    // while a modal passphrase prompt is still blocking apply startup.
    if apply_needs_vault_unlock(infra) {
        emit(SchedEvent::PrepareGlobal {
            message: "unlocking vault".into(),
        });
    }
    let interact_for_ssh = Arc::clone(&interact);
    let vault = infra.unlock_vault_session(interact).await?;
    if let Some(tx) = unlock_ready {
        let _ = tx.send(true);
    }
    let secrets = vault.secret_source();
    let prep = prepare_run_transports(infra, mode, events, Some(interact_for_ssh), secrets).await?;
    Ok(ApplyStartup { prep, vault })
}

fn apply_needs_vault_unlock(infra: &Infra) -> bool {
    infra.runtime.vault_store.is_some() && !infra.vault_data_keys.is_empty()
}

#[allow(clippy::too_many_arguments)]
async fn run_apply_prepared(
    prep: PreparedRun,
    vault: VaultSession,
    mode: RunMode,
    interact: Arc<dyn Interactor>,
    tui_events: Option<tokio::sync::broadcast::Sender<infrazeug_core::SchedEvent>>,
    commands_rx: Option<tokio::sync::mpsc::Receiver<infrazeug_core::SchedCommand>>,
    plan: Plan,
    methods: MethodRegistry,
) -> anyhow::Result<(RunReport, infrazeug_core::RunGuard, TestReport)> {
    let PreparedRun {
        prepared,
        guard,
        factory,
        test_report,
    } = prep;
    let native_executor = build_native_executor(
        &prepared.infra,
        methods,
        transport_as_native(Arc::clone(&factory)),
    );
    let executor: Arc<dyn OpExecutor> = factory;
    let result = prepared
        .infra
        .run_apply_with_guard_unlocked(
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
        .await;
    if let Ok((_, ref g)) = &result {
        teardown_containers(&prepared, g).await.ok();
    }
    result.map(|(r, g)| (r, g, test_report)).map_err(Into::into)
}

async fn run_apply_with_transports(
    bundle: &PlaybookBundle,
    mode: RunMode,
    interact: Arc<dyn Interactor>,
    tui_events: Option<tokio::sync::broadcast::Sender<infrazeug_core::SchedEvent>>,
    commands_rx: Option<tokio::sync::mpsc::Receiver<infrazeug_core::SchedCommand>>,
    plan: Plan,
) -> anyhow::Result<(RunReport, infrazeug_core::RunGuard, TestReport)> {
    let ApplyStartup { prep, vault } = prepare_apply_startup(
        &bundle.infra,
        mode,
        Arc::clone(&interact),
        tui_events.clone(),
        None,
    )
    .await?;
    run_apply_prepared(
        prep,
        vault,
        mode,
        interact,
        tui_events,
        commands_rx,
        plan,
        bundle.methods.clone(),
    )
    .await
}

pub async fn test_infra(
    bundle: &PlaybookBundle,
    opts: TestOptions,
) -> anyhow::Result<(RunReport, TestReport)> {
    if opts.dry_run {
        bundle.plan()?;
        print_dry_run_preview(bundle).await?;
        return Ok((RunReport::default(), TestReport::default()));
    }
    let plan_on_disk = opts.plan_path.as_deref().map(Plan::read_file).transpose()?;
    let plan = bundle
        .infra
        .resolve_plan(plan_on_disk.as_ref(), opts.force)?;
    let (events_tx, debug_task) = if opts.debug {
        let (tx, rx) = tokio::sync::broadcast::channel(4096);
        let task = tokio::spawn(crate::report_emit::debug_events_loop(rx));
        (Some(tx), Some(task))
    } else {
        (None, None)
    };
    let (report, guard, test_report) = run_apply_with_transports(
        bundle,
        RunMode::Test,
        Arc::new(NoPromptInteractor),
        events_tx,
        None,
        plan,
    )
    .await?;
    if let Some(task) = debug_task {
        task.await?;
    }
    guard.teardown().ok();
    Ok((report, test_report))
}

pub async fn apply_infra(infra: &Infra, opts: ApplyOptions) -> anyhow::Result<RunReport> {
    apply_bundle(&PlaybookBundle::from_infra(infra.clone()), opts).await
}

pub async fn apply_bundle(
    bundle: &PlaybookBundle,
    opts: ApplyOptions,
) -> anyhow::Result<RunReport> {
    if opts.dry_run {
        bundle.plan()?;
        print_dry_run_preview(bundle).await?;
        return Ok(RunReport::default());
    }

    if opts.emulate_first {
        let test_opts = TestOptions {
            force: opts.force,
            plan_path: opts.plan_path.clone(),
            unpinned: opts.unpinned,
            ..Default::default()
        };
        let (_, test_report) = test_infra(bundle, test_opts).await?;
        if !test_report.skipped.is_empty() {
            tracing::warn!(
                count = test_report.skipped.len(),
                "emulate-first: some machines skipped in test phase"
            );
        }
    }

    let plan_on_disk = opts.plan_path.as_deref().map(Plan::read_file).transpose()?;
    let plan = bundle
        .infra
        .resolve_plan(plan_on_disk.as_ref(), opts.force)?;

    if opts.tui {
        let (tui_interact, prompt_rx) = TuiInteractor::pair();
        let tui_interact: Arc<dyn Interactor> = tui_interact;
        let (events_tx, events_rx) = tokio::sync::broadcast::channel(4096);
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(16);
        let bundle = bundle.clone();
        let apply_handle = tokio::spawn(async move {
            let ApplyStartup { prep, vault } = prepare_apply_startup(
                &bundle.infra,
                RunMode::Apply,
                Arc::clone(&tui_interact),
                Some(events_tx.clone()),
                None,
            )
            .await?;
            run_apply_prepared(
                prep,
                vault,
                RunMode::Apply,
                tui_interact,
                Some(events_tx),
                Some(cmd_rx),
                plan,
                bundle.methods,
            )
            .await
        });
        let tui_result =
            infrazeug_tui::run_controller(events_rx, false, Some(prompt_rx), Some(cmd_tx)).await;
        let apply_result = apply_handle.await;
        tui_result?;
        let (report, guard, _) = apply_result??;
        guard.teardown().ok();
        Ok(report)
    } else if opts.watch {
        let interact: Arc<dyn Interactor> = Arc::new(NoPromptInteractor);
        let (events_tx, events_rx) = tokio::sync::broadcast::channel(4096);
        let bundle = bundle.clone();
        let apply_handle = tokio::spawn(async move {
            let ApplyStartup { prep, vault } = prepare_apply_startup(
                &bundle.infra,
                RunMode::Apply,
                Arc::clone(&interact),
                Some(events_tx.clone()),
                None,
            )
            .await?;
            run_apply_prepared(
                prep,
                vault,
                RunMode::Apply,
                interact,
                Some(events_tx),
                None,
                plan,
                bundle.methods,
            )
            .await
        });
        infrazeug_tui::run_controller(events_rx, true, None, None).await?;
        let (report, guard, _) = apply_handle.await??;
        guard.teardown().ok();
        Ok(report)
    } else {
        let interact: Arc<dyn Interactor> = if atty::is(atty::Stream::Stdin) {
            Arc::new(LineInteractor)
        } else {
            Arc::new(AutoDenyInteractor)
        };
        let (events_tx, debug_task, unlock_ready) = if opts.debug {
            let (tx, rx) = tokio::sync::broadcast::channel(4096);
            let (ready_tx, ready_rx) = tokio::sync::watch::channel(false);
            let task = tokio::spawn(crate::report_emit::debug_events_loop_after_unlock(
                rx, ready_rx,
            ));
            (Some(tx), Some(task), Some(ready_tx))
        } else if apply_needs_vault_unlock(&bundle.infra) {
            let (tx, _rx) = tokio::sync::broadcast::channel(16);
            (Some(tx), None, None)
        } else {
            (None, None, None)
        };
        let ApplyStartup { prep, vault } = prepare_apply_startup(
            &bundle.infra,
            RunMode::Apply,
            Arc::clone(&interact),
            events_tx.clone(),
            unlock_ready,
        )
        .await?;
        let (report, guard, _) = run_apply_prepared(
            prep,
            vault,
            RunMode::Apply,
            interact,
            events_tx,
            None,
            plan,
            bundle.methods.clone(),
        )
        .await?;
        if let Some(task) = debug_task {
            task.await?;
        }
        guard.teardown().ok();
        Ok(report)
    }
}

pub fn default_infra() -> Infra {
    Infra::new().with_scheduler(Arc::new(DefaultScheduler))
}

/// Run an ad-hoc `infra` (built by an MCP tool) through the real apply pipeline
/// and collect its run report together with every node's captured stdout.
///
/// Used by [`mcp_serve::ApiExecutor`]; never gathers or returns secrets.
pub(crate) async fn run_infra_collect(
    bundle: &PlaybookBundle,
) -> anyhow::Result<infrazeug_mcp::ToolRun> {
    use infrazeug_core::CaptureStore;

    let plan = bundle.infra.resolve_plan(None, false)?;
    let prep = prepare_run_transports(&bundle.infra, RunMode::Apply, None, None, None).await?;
    let PreparedRun {
        prepared,
        guard,
        factory,
        test_report: _,
    } = prep;
    let captures = Arc::new(CaptureStore::new());
    let native_executor = build_native_executor(
        &prepared.infra,
        bundle.methods.clone(),
        transport_as_native(Arc::clone(&factory)),
    );
    let executor: Arc<dyn OpExecutor> = factory;
    let (report, guard) = prepared
        .infra
        .run_apply_with_guard_captures(
            RunMode::Apply,
            Arc::new(NoPromptInteractor),
            None,
            None,
            plan,
            executor,
            native_executor,
            guard,
            Arc::clone(&captures),
        )
        .await?;
    teardown_containers(&prepared, &guard).await.ok();
    guard.teardown().ok();

    let map = captures.lookup_map().await?;
    let captures = map
        .into_iter()
        .map(|((node, machine), bytes)| infrazeug_mcp::CaptureOut {
            node: prepared
                .infra
                .nodes
                .iter()
                .find(|n| n.id.0 == node)
                .map(|n| n.name.to_string())
                .unwrap_or_else(|| node.to_string()),
            machine: prepared
                .infra
                .machine_by_id(infrazeug_core::MachineId(machine))
                .map(|m| m.name.clone())
                .unwrap_or_else(|| machine.to_string()),
            stdout: String::from_utf8_lossy(&bytes).into_owned(),
        })
        .collect();
    Ok(infrazeug_mcp::ToolRun { report, captures })
}

pub mod builder {
    use super::*;
    use infrazeug_core::{resolve_machine_typed, Node};
    use infrazeug_shell::ShellOp;
    use std::marker::PhantomData;
    use uuid::Uuid;

    /// A group handle carrying the Rust var-schema type `V` for its machines.
    ///
    /// Targeting a typed group (via [`InfraBuilder::on_group`]) resolves each
    /// member machine's effective vars into `V` and hands `&V` to the body
    /// closure, so `template!` field references are type-checked against `V`.
    pub struct TypedGroup<V> {
        pub id: GroupId,
        _p: PhantomData<fn() -> V>,
    }

    impl<V> TypedGroup<V> {
        /// Attach the var-schema type `V` to an already-registered group id.
        pub fn new(id: GroupId) -> Self {
            Self {
                id,
                _p: PhantomData,
            }
        }
    }

    impl<V> Clone for TypedGroup<V> {
        fn clone(&self) -> Self {
            *self
        }
    }
    impl<V> Copy for TypedGroup<V> {}

    pub struct InfraBuilder {
        inner: Infra,
        methods: MethodRegistry,
    }

    /// Inject a per-machine [`infrazeug_core::node::NodeBody::Connect`] head node so
    /// agent upload / SSH reachability becomes an explicit in-graph step (the
    /// replacement for the eager pre-apply agent phase), and make every machine
    /// lazy so the connect node owns first transport use.
    ///
    /// Every machine-root node — one with no existing dependency that already runs
    /// on that machine — gains a dependency on its machine's connect node, so the
    /// connect node is a transitive ancestor of all that machine's work without
    /// over-constraining intra-machine chains. The connect node reports `Changed`,
    /// preserving the "runs every apply" behavior such roots had as graph roots.
    fn inject_connectivity_nodes(infra: &mut Infra) {
        use infrazeug_core::id::{MachineId, NodeId};
        use infrazeug_core::node::{NodeBody, Targets};
        use std::collections::{HashMap, HashSet};

        let machine_ids: Vec<MachineId> = infra.machines.iter().map(|m| m.id).collect();
        if machine_ids.is_empty() {
            return;
        }
        let connect_of: HashMap<MachineId, NodeId> = machine_ids
            .iter()
            .map(|&m| (m, infrazeug_core::connect_node_id(m)))
            .collect();

        // Resolve each existing node's target machine set once.
        let node_machines: HashMap<NodeId, HashSet<MachineId>> = infra
            .nodes
            .iter()
            .map(|n| {
                let set: HashSet<MachineId> = infra
                    .resolve_targets(&n.targets)
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                (n.id, set)
            })
            .collect();

        let node_is_graph_only: HashMap<NodeId, bool> = infra
            .nodes
            .iter()
            .map(|n| (n.id, n.body.is_graph_only()))
            .collect();
        let deps_of: HashMap<NodeId, Vec<NodeId>> =
            infra.nodes.iter().map(|n| (n.id, n.deps.clone())).collect();

        fn covered_machines(
            node_id: NodeId,
            node_is_graph_only: &HashMap<NodeId, bool>,
            deps_of: &HashMap<NodeId, Vec<NodeId>>,
            node_machines: &HashMap<NodeId, HashSet<MachineId>>,
            memo: &mut HashMap<NodeId, HashSet<MachineId>>,
            visiting: &mut HashSet<NodeId>,
        ) -> HashSet<MachineId> {
            if let Some(cached) = memo.get(&node_id) {
                return cached.clone();
            }
            if !visiting.insert(node_id) {
                return HashSet::new();
            }

            let mut covered = HashSet::new();
            if !node_is_graph_only
                .get(&node_id)
                .copied()
                .unwrap_or_default()
            {
                if let Some(machines) = node_machines.get(&node_id) {
                    covered.extend(machines.iter().copied());
                }
            } else {
                for dep in deps_of.get(&node_id).into_iter().flatten() {
                    covered.extend(covered_machines(
                        *dep,
                        node_is_graph_only,
                        deps_of,
                        node_machines,
                        memo,
                        visiting,
                    ));
                }
                if let Some(machines) = node_machines.get(&node_id) {
                    covered.extend(machines.iter().copied());
                }
            }

            visiting.remove(&node_id);
            memo.insert(node_id, covered.clone());
            covered
        }

        // For each eager node, add connect deps for the target machines not already
        // covered by existing dependencies. Transport-using deps cover their own
        // target machines. Graph-only deps (barriers/group bookends) forward the
        // coverage of their deps plus their own injected connect heads, so the DAG
        // remains connected as `connect/* -> begin/barrier -> real work`.
        let mut coverage_memo = HashMap::new();
        let mut add_deps: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        for n in &infra.nodes {
            if matches!(n.body, NodeBody::Connect) || matches!(n.policy.run_policy, RunPolicy::Lazy)
            {
                continue;
            }
            let mine = &node_machines[&n.id];
            let mut covered: HashSet<MachineId> = HashSet::new();
            for dep in &n.deps {
                covered.extend(covered_machines(
                    *dep,
                    &node_is_graph_only,
                    &deps_of,
                    &node_machines,
                    &mut coverage_memo,
                    &mut HashSet::new(),
                ));
            }
            let adds: Vec<NodeId> = mine
                .iter()
                .filter(|m| !covered.contains(m))
                .filter_map(|m| connect_of.get(m).copied())
                .collect();
            if !adds.is_empty() {
                add_deps.insert(n.id, adds);
            }
        }
        for n in &mut infra.nodes {
            if let Some(adds) = add_deps.remove(&n.id) {
                for cid in adds {
                    if !n.deps.contains(&cid) {
                        n.deps.push(cid);
                    }
                }
            }
        }

        // Every machine now connects on first node use.
        for m in &mut infra.machines {
            m.lazy = true;
        }

        // Append the execution start and connect head nodes (deterministic ids,
        // single-machine targets). The start node is real scheduler input, not a
        // renderer artifact, so graph traversal and execution ordering agree.
        let control_machine = infra
            .machines
            .iter()
            .find(|m| matches!(m.kind, MachineKind::Local | MachineKind::Container(_)))
            .or_else(|| infra.machines.first())
            .map(|m| m.id)
            .expect("machine_ids is non-empty");
        let start_id = infrazeug_core::start_node_id();
        if !infra.nodes.iter().any(|n| n.id == start_id) {
            infra
                .nodes
                .push(infrazeug_core::start_node_on(control_machine));
        }
        let names: HashMap<MachineId, String> = infra
            .machines
            .iter()
            .map(|m| (m.id, format!("connect/{}", m.name)))
            .collect();
        for &mid in &machine_ids {
            let name = names.get(&mid).cloned().unwrap_or_else(|| "connect".into());
            let node = infrazeug_core::connect_node(
                connect_of[&mid],
                name,
                Targets::Machine(mid),
                vec![start_id],
            );
            infra.nodes.push(node);
        }

        // Add a real graph-only sink. It deliberately ignores lazy leaves so
        // dormant lazy chains are not demanded just because the graph has an end.
        let end_id = infrazeug_core::end_node_id();
        if !infra.nodes.iter().any(|n| n.id == end_id) {
            let depended_on: HashSet<NodeId> = infra
                .nodes
                .iter()
                .flat_map(|n| n.deps.iter().copied())
                .collect();
            let deps: Vec<NodeId> = infra
                .nodes
                .iter()
                .filter(|n| n.id != end_id)
                .filter(|n| !matches!(n.policy.run_policy, RunPolicy::Lazy))
                .filter(|n| !depended_on.contains(&n.id))
                .map(|n| n.id)
                .collect();
            infra
                .nodes
                .push(infrazeug_core::end_node_on(control_machine, deps));
        }
    }

    /// Staged shell-node helper returned by [`InfraBuilder::shell_node`].
    pub struct ShellNodeBuilder {
        builder: InfraBuilder,
        node_id: NodeId,
        machine_id: MachineId,
        op: ShellOp,
        name: Option<String>,
        description: Option<String>,
        change_policy: OutputChangePolicy,
        deps: Vec<NodeId>,
        run_policy: RunPolicy,
    }

    impl ShellNodeBuilder {
        pub fn name(mut self, name: &str) -> Self {
            self.name = Some(name.into());
            self
        }

        pub fn description(mut self, description: &str) -> Self {
            self.description = Some(description.into());
            self
        }

        /// Replace the node's successful-output change classifier.
        ///
        /// Intent: let shell nodes that exit `0` for both "changed" and
        /// "nothing to do" still drive graph propagation accurately from
        /// stable stdout/stderr markers.
        ///
        /// Empty/default policy keeps shell semantics where exit `0` reports
        /// changed. Non-zero exits are still failures/retries.
        pub fn change_policy(mut self, change_policy: OutputChangePolicy) -> Self {
            self.change_policy = change_policy;
            self
        }

        /// Mark this shell node changed when successful output contains `needle`.
        ///
        /// Rules are checked in insertion order. Use this before broader
        /// unchanged rules when the command can print both kinds of markers.
        pub fn changed_when_contains(
            mut self,
            stream: OutputMatchStream,
            needle: impl Into<String>,
        ) -> Self {
            self.change_policy
                .rules
                .push(OutputChangeRule::changed_when_contains(stream, needle));
            self
        }

        /// Mark this shell node unchanged when successful output contains `needle`.
        ///
        /// This is intended for idempotent shell commands that exit `0` even
        /// when they did nothing. A classified `Unchanged` result does not fire
        /// default `OnUpstreamChange` successors.
        pub fn unchanged_when_contains(
            mut self,
            stream: OutputMatchStream,
            needle: impl Into<String>,
        ) -> Self {
            self.change_policy
                .rules
                .push(OutputChangeRule::unchanged_when_contains(stream, needle));
            self
        }

        pub fn deps(mut self, deps: impl IntoIterator<Item = NodeId>) -> Self {
            self.deps.extend(deps);
            self
        }

        /// Wire this node as the next member of a [`SyncNodeGroup`].
        pub fn in_sync_group(mut self, group: &SyncNodeGroup) -> Self {
            self.deps.extend(group.next_deps());
            self
        }

        /// Wire this node as a parallel member of an [`AsyncNodeGroup`].
        pub fn in_async_group(mut self, group: &AsyncNodeGroup) -> Self {
            self.deps.extend(group.next_deps());
            self
        }

        pub fn on_upstream_change(mut self) -> Self {
            self.run_policy = RunPolicy::OnUpstreamChange;
            self
        }

        /// Run on every apply (e.g. reconcile a config-derived value), regardless of
        /// whether upstream nodes changed.
        pub fn always(mut self) -> Self {
            self.run_policy = RunPolicy::Always;
            self
        }

        pub fn build(mut self) -> anyhow::Result<InfraBuilder> {
            let name = self.name.unwrap_or_else(|| self.node_id.to_string());
            let mut node =
                NodeBuilder::shell(self.node_id, self.op, Targets::Machine(self.machine_id))
                    .name(name)
                    .deps(self.deps)
                    .run_policy(self.run_policy)
                    .build();
            if let Some(d) = self.description {
                node = node.with_description(d);
            }
            node.policy.success.change_policy = self.change_policy;
            self.builder.inner = self.builder.inner.add_node(node)?;
            Ok(self.builder)
        }
    }

    impl Default for InfraBuilder {
        fn default() -> Self {
            Self::new()
        }
    }

    impl InfraBuilder {
        pub fn new() -> Self {
            Self {
                inner:
                    default_infra().with_default_remote_transport(
                        crate::transport_env::default_remote_transport(),
                    ),
                methods: MethodRegistry::new(),
            }
        }

        /// Register a [`NodeMethod`] for this playbook (required before [`native_typed`](Self::native_typed)).
        pub fn method<M: NodeMethod + 'static>(mut self, method: M) -> Self {
            self.methods.register(method);
            self
        }

        /// Alias for [`method`](Self::method).
        pub fn register_method<M: NodeMethod + 'static>(self, method: M) -> Self {
            self.method(method)
        }

        /// Add a native node on a machine, registering `method` and using its [`NodeMethod::name`].
        pub fn native<M: NodeMethod + 'static>(
            self,
            node_id: NodeId,
            machine_id: MachineId,
            method: M,
            input: M::Input,
        ) -> anyhow::Result<native_builder::NativeNodeBuilder> {
            self.native_named(node_id, method.name(), machine_id, method, input)
        }

        /// Like [`native`](Self::native) with an explicit node display name.
        pub fn native_named<M: NodeMethod + 'static>(
            self,
            node_id: NodeId,
            name: &str,
            machine_id: MachineId,
            method: M,
            input: M::Input,
        ) -> anyhow::Result<native_builder::NativeNodeBuilder> {
            native_builder::native_node_with_method(self, node_id, name, machine_id, method, input)
        }

        /// Add a native node for a method already registered via [`method`](Self::method).
        pub fn native_typed<M: NodeMethod + 'static>(
            self,
            node_id: NodeId,
            name: &str,
            machine_id: MachineId,
            input: M::Input,
        ) -> anyhow::Result<native_builder::NativeNodeBuilder> {
            native_builder::native_node_typed::<M>(self, node_id, name, machine_id, input)
        }

        /// Shorthand: [`native`](Self::native) on a local/controller machine, then [`NativeNodeBuilder::build`].
        pub fn native_on_local<M: NodeMethod + 'static>(
            self,
            node_id: NodeId,
            name: &str,
            machine_id: MachineId,
            method: M,
            input: M::Input,
        ) -> anyhow::Result<Self> {
            self.native_named(node_id, name, machine_id, method, input)?
                .build()
        }

        pub fn global_vars(mut self, vars: VarSet) -> Self {
            self.inner = self.inner.with_global_vars(vars);
            self
        }

        pub fn vault_data_keys(mut self, keys: Vec<String>) -> Self {
            self.inner = self.inner.with_vault_data_keys(keys);
            self
        }

        pub fn group(mut self, group: Group) -> anyhow::Result<Self> {
            self.inner = self.inner.add_group(group)?;
            Ok(self)
        }

        pub fn machine(mut self, machine: Machine) -> anyhow::Result<Self> {
            self.inner = self.inner.add_machine(machine)?;
            Ok(self)
        }

        pub fn node(mut self, node: Node) -> anyhow::Result<Self> {
            self.inner = self.inner.add_node(node)?;
            Ok(self)
        }

        pub(crate) fn add_built_node(mut self, node: Node) -> anyhow::Result<Self> {
            self.inner = self.inner.add_node(node)?;
            Ok(self)
        }

        pub(crate) fn register_native_method<M: NodeMethod + 'static>(&mut self, method: M) {
            self.methods.register(method);
        }

        pub(crate) fn add_dynamic_group(&mut self, group: infrazeug_core::dynamic::DynamicGroup) {
            self.inner.push_dynamic_group(group);
        }

        pub(crate) fn method_name<M: NodeMethod + 'static>(&self) -> Option<&str> {
            self.methods.name_of::<M>()
        }

        pub fn barrier(
            self,
            node_id: NodeId,
            name: &str,
            targets: Targets,
            deps: Vec<NodeId>,
        ) -> anyhow::Result<Self> {
            self.barrier_desc(node_id, name, None, targets, deps)
        }

        /// Add a graph-only dependency barrier.
        ///
        /// Barriers perform no remote work. They preserve dependency ordering
        /// and propagate meaningful upstream changes without shell sentinels.
        pub fn barrier_desc(
            mut self,
            node_id: NodeId,
            name: &str,
            description: Option<&str>,
            targets: Targets,
            deps: Vec<NodeId>,
        ) -> anyhow::Result<Self> {
            let mut node = barrier_node(node_id, name.to_string(), targets, deps);
            if let Some(d) = description {
                node = node.with_description(d);
            }
            self.inner = self.inner.add_node(node)?;
            Ok(self)
        }

        pub fn barrier_on_machine(
            self,
            node_id: NodeId,
            name: &str,
            machine_id: MachineId,
            deps: Vec<NodeId>,
        ) -> anyhow::Result<Self> {
            self.barrier(node_id, name, Targets::Machine(machine_id), deps)
        }

        pub fn shell_on_local(
            self,
            node_id: NodeId,
            name: &str,
            machine_id: MachineId,
            op: ShellOp,
        ) -> anyhow::Result<Self> {
            self.shell_on_machine(node_id, name, machine_id, op)
        }

        pub fn shell_on_machine(
            self,
            node_id: NodeId,
            name: &str,
            machine_id: MachineId,
            op: ShellOp,
        ) -> anyhow::Result<Self> {
            self.shell_on_machine_desc(node_id, name, None, machine_id, op)
        }

        /// Like [`shell_on_machine`](Self::shell_on_machine) with an optional description.
        pub fn shell_on_machine_desc(
            mut self,
            node_id: NodeId,
            name: &str,
            description: Option<&str>,
            machine_id: MachineId,
            op: ShellOp,
        ) -> anyhow::Result<Self> {
            let mut node = shell_node(node_id, name.to_string(), op, Targets::Machine(machine_id));
            if let Some(d) = description {
                node = node.with_description(d);
            }
            self.inner = self.inner.add_node(node)?;
            Ok(self)
        }

        /// Add a shell node built with [`NodeBuilder`] (name and description optional).
        pub fn shell_node(
            self,
            node_id: NodeId,
            machine_id: MachineId,
            op: ShellOp,
        ) -> ShellNodeBuilder {
            ShellNodeBuilder {
                builder: self,
                node_id,
                machine_id,
                op,
                name: None,
                description: None,
                change_policy: OutputChangePolicy::default(),
                deps: Vec::new(),
                run_policy: RunPolicy::default(),
            }
        }

        /// Look up an already-registered group by name and attach the var-schema
        /// type `V`. Add the group (with `.group(..)`) before calling this.
        pub fn typed_group<V>(&self, name: &str) -> anyhow::Result<TypedGroup<V>> {
            let g = self
                .inner
                .groups
                .iter()
                .find(|g| g.name.as_str() == name)
                .ok_or_else(|| anyhow::anyhow!("group `{name}` not registered"))?;
            Ok(TypedGroup::new(g.id))
        }

        /// Add one node per machine in the typed group, rendering its body from
        /// that machine's vars resolved into `V` (SOUL §3.9 precedence).
        ///
        /// `body` receives the concrete `Machine` and `&V`; it returns the
        /// `ShellOp`s for that machine (e.g. `write_rendered(.., template!(..))`).
        /// Per-machine `NodeId`s are derived deterministically from the group
        /// name, machine id, and `node_name`, so plans stay stable across runs.
        pub fn on_group<V, F>(
            mut self,
            group: TypedGroup<V>,
            node_name: &str,
            body: F,
        ) -> anyhow::Result<Self>
        where
            V: serde::de::DeserializeOwned,
            F: Fn(&Machine, &V) -> Vec<ShellOp>,
        {
            let group_name = self
                .inner
                .group(group.id)
                .ok_or_else(|| anyhow::anyhow!("group {:?} not registered", group.id))?
                .name
                .clone();

            let machine_ids: Vec<MachineId> = self
                .inner
                .machines
                .iter()
                .filter(|m| m.groups.contains(&group.id))
                .map(|m| m.id)
                .collect();

            for mid in machine_ids {
                let machine = self
                    .inner
                    .machine_by_id(mid)
                    .expect("machine id came from this infra")
                    .clone();
                let v: V = resolve_machine_typed(
                    &self.inner.global_vars,
                    &self.inner.groups,
                    &machine,
                    None,
                )?;
                let ops = body(&machine, &v);
                let op = match ops.len() {
                    1 => ops.into_iter().next().unwrap(),
                    _ => ShellOp::Seq { steps: ops },
                };
                let seed = format!("infrazeug/on_group/{group_name}/{}/{node_name}", mid.0);
                let node_id = NodeId(Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes()));
                let node = shell_node(
                    node_id,
                    format!("{node_name}@{}", machine.name),
                    op,
                    Targets::Machine(mid),
                );
                self.inner = self.inner.add_node(node)?;
            }
            Ok(self)
        }

        pub fn default_remote_transport(mut self, choice: TransportChoice) -> Self {
            self.inner = self.inner.with_default_remote_transport(choice);
            self
        }

        /// Insert the programmatic begin node for a sync group.
        pub fn begin_sync_group(
            mut self,
            group: &mut SyncNodeGroup,
            targets: Targets,
        ) -> anyhow::Result<Self> {
            self.inner.begin_sync_group(group, targets)?;
            Ok(self)
        }

        /// Insert the programmatic begin node for an async group.
        pub fn begin_async_group(
            mut self,
            group: &mut AsyncNodeGroup,
            targets: Targets,
        ) -> anyhow::Result<Self> {
            self.inner.begin_async_group(group, targets)?;
            Ok(self)
        }

        /// Insert the programmatic finish node for a sync group.
        pub fn finish_sync_group(
            mut self,
            group: &mut SyncNodeGroup,
            targets: Targets,
        ) -> anyhow::Result<(Self, NodeId)> {
            let exit = self.inner.finish_sync_group(group, targets)?;
            Ok((self, exit))
        }

        /// Insert the programmatic finish node for an async group.
        pub fn finish_async_group(
            mut self,
            group: &mut AsyncNodeGroup,
            targets: Targets,
        ) -> anyhow::Result<(Self, NodeId)> {
            let exit = self.inner.finish_async_group(group, targets)?;
            Ok((self, exit))
        }

        pub fn build(self) -> PlaybookBundle {
            let mut infra = self.inner;
            inject_connectivity_nodes(&mut infra);
            PlaybookBundle {
                infra,
                methods: self.methods,
            }
        }

        pub fn build_infra(self) -> Infra {
            self.build().infra
        }
    }

    /// Build a `WriteFile` op from already-rendered template output (e.g. the
    /// `String` returned by `template!`). Bytes are inline, so the op lowers,
    /// hashes, and seals for pull-mode like any other `FileSource::Bytes`.
    pub fn write_rendered(
        path: impl Into<std::path::PathBuf>,
        mode: u32,
        content: String,
    ) -> ShellOp {
        ShellOp::write_file(path, FileSource::bytes(content.into_bytes()), mode)
    }

    pub fn local(id: MachineId, name: &str) -> Machine {
        local_machine(id, name.to_string())
    }

    /// Local machine for controller-side native/API work (alias for [`local`](Self::local)(id, `"controller"`)).
    pub fn controller(id: MachineId) -> Machine {
        local_machine(id, "controller")
    }

    pub fn remote(id: MachineId, name: &str, ssh: SshConfig) -> Machine {
        remote_machine(id, name.to_string(), ssh)
    }

    pub use crate::dynamic::{DynamicGroupBuilder, MachineTemplate};
    pub use crate::native_builder::NativeNodeBuilder;
    pub use infrazeug_core::{
        begin_node_id, finish_node_id, AsyncNodeGroup, NodeBuilder, SyncNodeGroup,
    };
}
