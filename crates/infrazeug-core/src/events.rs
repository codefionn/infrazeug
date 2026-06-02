use crate::id::{MachineId, NodeId};
use crate::machine::MachineSummary;
use crate::node::NodeSummary;
use infrazeug_shell::OutputStream;
use serde_json::Value;
use std::time::Duration;

/// Per-machine resource-usage sample, forwarded from the push-mode agent's
/// out-of-band metrics stream. Byte counts are absolute; `cpu_pct` is 0.0–100.0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MachineMetrics {
    pub cpu_pct: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub disk_used: u64,
    pub disk_total: u64,
}

/// Per-machine transport bootstrap before apply (agent probe, build, upload, connect).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MachinePreparePhase {
    Pending,
    ProbingArch,
    BuildingAgent,
    UploadingAgent,
    Connecting,
    Ready,
    Skipped { reason: String },
    Failed { message: String },
}

#[derive(Clone, Debug)]
pub enum SchedEvent {
    /// Begin transport prepare; seeds the TUI machine grid before apply units exist.
    PrepareStarted {
        machine_summaries: Vec<(MachineId, MachineSummary)>,
    },
    /// Controller-wide prepare step (emulation, agent build, transport connect).
    PrepareGlobal {
        message: String,
    },
    PrepareMachine {
        machine: MachineId,
        phase: MachinePreparePhase,
        detail: Option<String>,
    },
    PrepareFinished {
        ok: bool,
        message: Option<String>,
    },
    NodeQueued {
        node: NodeId,
        machine: MachineId,
    },
    NodeStarted {
        node: NodeId,
        machine: MachineId,
    },
    NodeProgress {
        node: NodeId,
        machine: MachineId,
        message: String,
    },
    NodeOutput {
        node: NodeId,
        machine: MachineId,
        stream: OutputStream,
        data: Vec<u8>,
    },
    NodeRetrying {
        node: NodeId,
        machine: MachineId,
        attempt: u32,
        max_attempts: u32,
        message: String,
    },
    NodeReconnecting {
        node: NodeId,
        machine: MachineId,
        attempt: u32,
        message: String,
    },
    NodePolling {
        node: NodeId,
        machine: MachineId,
        message: String,
    },
    NodeFinished {
        node: NodeId,
        machine: MachineId,
        status: crate::node::NodeStatus,
        duration: Duration,
    },
    NodeCancelled {
        node: NodeId,
        machine: MachineId,
        reason: String,
    },
    /// Out-of-band resource-usage sample pushed by a push-mode agent. Not tied
    /// to any node; arrives on a timer for as long as the agent is connected.
    MachineMetrics {
        machine: MachineId,
        metrics: MachineMetrics,
    },
    PlanWarning {
        message: String,
    },
    RunStarted {
        total_units: usize,
        planned_by_machine: Vec<(MachineId, usize)>,
        machine_summaries: Vec<(MachineId, MachineSummary)>,
        node_summaries: Vec<(NodeId, NodeSummary)>,
    },
    RunFinished {
        total_units: usize,
        succeeded: usize,
        failed: usize,
        cancelled: usize,
    },
    /// Dynamic-group fan-out: new (node × machine) units were added mid-run after a
    /// discovery node resolved its machines. Controllers grow their totals/grids.
    UnitsAdded {
        added_units: usize,
        planned_by_machine: Vec<(MachineId, usize)>,
        machine_summaries: Vec<(MachineId, MachineSummary)>,
        node_summaries: Vec<(NodeId, NodeSummary)>,
    },
}

#[derive(Clone, Debug)]
pub enum SchedCommand {
    CancelNode {
        node: NodeId,
        machine: MachineId,
        grace: Duration,
    },
    CancelMachine {
        machine: MachineId,
    },
    PauseAll,
    ResumeAll,
    ReplayNode {
        node: NodeId,
        machine: MachineId,
    },
    /// Visual filter selector (tag expression). Controller-side only — does
    /// not affect execution (§6ter.7).
    FilterChange {
        selector: String,
    },
}

#[derive(Clone, Debug)]
pub enum ProgressKind {
    LogLine,
    Other,
}

#[allow(dead_code)]
pub struct ProgressPayload(pub Value);
