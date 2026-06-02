//! Core graph, scheduler, plan/apply, and variable model (SOUL §3).
//!
//! This crate is the execution engine: embedders use it directly or through
//! [`infrazeug_api`]. It owns the [`Infra`] graph, [`Machine`] targets, [`Node`]
//! DAG, edge-aware [`Scheduler`], deterministic plan/apply, and [`VarSet`]
//! precedence (groups, machine vars, tags).
//!
//! # Main modules
//!
//! | Module | Role |
//! |--------|------|
//! | [`infra`] | Graph builder: machines, nodes, groups, lifecycle |
//! | [`scheduler`] | Concurrent apply, barriers, per-machine workers |
//! | [`plan`] / [`exec`] | Plan-time checks and [`OpExecutor`] runtime |
//! | [`graph`] | DAG view, selection, dependency edges |
//! | [`varset`] / [`vault_resolve`] | Layered variables and vault field paths |
//! | [`interactor`] | Unlock/approval protocol (CLI, TUI, tests) |
//! | [`lint`] | Collect-all diagnostics for `lint` |
//! | [`slice`] | Pull-mode [`PlanSlice`] sealing constraints |
//!
//! Transport I/O (SSH, agent push, local shell) lives in `infrazeug-transport`;
//! this crate only carries [`TransportChoice`] and scheduling hooks.
//!
//! [`Infra`]: infra::Infra
//! [`Machine`]: machine::Machine
//! [`Node`]: node::Node
//! [`Scheduler`]: scheduler::Scheduler
//! [`VarSet`]: varset::VarSet
//! [`infrazeug_api`]: infrazeug_api

pub mod capture;
pub mod control;
pub mod decision;
pub mod dynamic;
pub mod error;
pub mod events;
pub mod exec;
pub mod execution_graph;
pub mod graph;
pub mod hash_relay;
pub mod id;
pub mod infra;
pub mod interactor;
pub mod limits;
pub mod lint;
pub mod locks;
pub mod machine;
pub mod native_exec;
pub mod node;
pub mod node_group;
pub mod output;
pub mod passphrase_io;
pub mod plan;
pub mod report;
pub mod retry;
pub mod runtime;
pub mod scheduler;
pub mod secret_scan;
pub mod slice;
pub mod ssh_askpass;
pub mod test_mode;
pub mod transport;
pub mod var_serve;
pub mod varset;
pub mod vault_resolve;

pub use capture::{resolve_op_captures, validate_capture_refs, CaptureStore};
pub use decision::{GraphState, SkipReason, UnitDecision};
pub use dynamic::{
    compile_expansion, dyn_exit_node_id, dyn_instance_node_id, dyn_machine_id,
    template_placeholder_machine, DynamicExpansion, DynamicGroup,
};
pub use error::{CoreError, Result};
pub use events::{MachinePreparePhase, SchedCommand, SchedEvent};
pub use exec::{LocalExecutor, OpExecutor};
pub use execution_graph::{
    ExecAction, ExecNode, ExecutionGraph, ExecutionGraphPatch, NodeAction, NoopRole,
    SchedulerCompat, SystemAction, WorkKey, WorkUnit,
};
pub use graph::{GraphEdge, GraphNode, GraphSelect, GraphView};
pub use hash_relay::HashRelay;
pub use id::{uuid, GroupId, MachineId, NodeId, RunId, Tag};
pub use infra::{
    barrier_node, begin_node, connect_node, connect_node_id, end_node, end_node_id, end_node_on,
    finish_node, start_node, start_node_id, start_node_on, Infra,
};
pub use infrazeug_native::{MethodRegistry, NativeResult, NativeStatus, NodeMethod};
pub use infrazeug_secrets::{PlanSignature, VaultRef};
pub use infrazeug_shell::{OutputChunk, OutputStream};
pub use interactor::{
    AutoDenyInteractor, Interaction, InteractionResp, Interactor, LineInteractor,
};
pub use limits::GlobalLimits;
pub use lint::{Diagnostic, LintReport, Severity};
pub use locks::LockBag;
pub use machine::{
    AddressFamily, Group, Machine, MachineKind, MachineSpec, MachineSummary, OsFamily, OsHint,
    SshAuth, SshConfig, SshSecret,
};
pub use native_exec::{
    empty_native_executor, native_supported_on_kind, EmptyNativeExecutor, LocalNativeExecutor,
    NativeExecutor, RoutingNativeExecutor,
};
pub use node::{
    short_id_prefix, FailPolicy, LockPolicy, Node, NodeBody, NodeBuilder, NodePolicy, NodeStatus,
    NodeSummary, OutputChangePolicy, OutputChangeRule, OutputMatchStatus, OutputMatchStream,
    PlanOutcome, PostRunPolicy, RunPolicy, SuccessPolicy, Targets,
};
pub use node_group::{
    begin_node_id, finish_member_deps, finish_node_id, AsyncNodeGroup, SyncNodeGroup,
};
pub use output::{format_to_string, OutputFormat};
pub use passphrase_io::read_passphrase_prompt;
pub use plan::{
    map_plan_outcome, node_fingerprint, plan_digest, ExecutablePlan, NodeFingerprint, Plan,
    PlanDigest, PlannedNode, Preview, PreviewCounts, PreviewNode,
};
pub use report::{RunReport, RunReportEntry, TestReport};
pub use retry::{Backoff, PollCheck, PollConfig, ReconnectConfig, RetryConfig, RetryMode};
pub use runtime::{run_dir_name, RunGuard, RunMode, RuntimeConfig, VaultSession};
pub use scheduler::{DefaultScheduler, SchedRuntime, Scheduler};
pub use secret_scan::collect_plaintext_secrets;
pub use slice::{
    slice_digest, slice_to_plan, PlanSlice, Sha256Digest, SliceMode, SliceStep, WaitId,
};
pub use test_mode::{expand_machines, EffectiveMachine};
pub use transport::TransportChoice;
pub use var_serve::{ApprovalKey, VarServeState};
pub use varset::{
    resolve_machine, resolve_machine_typed, ResolvedVar, VarAcl, VarKey, VarSet, VarSource,
    VarValue,
};
pub use vault_resolve::resolve_vault_in_shell_op;
