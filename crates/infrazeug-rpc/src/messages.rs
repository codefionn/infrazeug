use infrazeug_native::NativeResult;
use infrazeug_shell::local::{ExecOutput, OutputStream};
use infrazeug_shell::ShellOp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RpcNodeStatus {
    Pending,
    Running,
    Changed,
    Unchanged,
    Skipped,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcNodeGraphEntry {
    pub node_id: Uuid,
    pub status: RpcNodeStatus,
}

/// RPC requests sent from the controller to the push-mode agent over the
/// postcard-stdio microarchitecture (see `docs/protocol.md`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RpcRequest {
    Ping,
    ExecuteShell {
        op: ShellOp,
    },
    SyncNodeGraphState {
        completed: Vec<RpcNodeGraphEntry>,
    },
    VarRequest {
        node_id: Uuid,
        machine_id: Uuid,
        var_key: String,
        plan_digest: [u8; 32],
    },
    ExecuteNative {
        method_id: String,
        input: serde_cbor::Value,
    },
}

/// RPC responses returned by the push-mode agent to the controller.
///
/// `ExecuteShell` may produce zero or more output chunk frames before the
/// final `ExecResult`; other requests still produce one response frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RpcResponse {
    Pong,
    ExecOutputChunk { stream: OutputStream, data: Vec<u8> },
    ExecResult(ExecOutput),
    NodeGraphStateSynced,
    VarValue(serde_json::Value),
    VarDenied { reason: String },
    NativeResult(NativeResult),
    Error(String),
}

/// Resource-usage sample the agent pushes out-of-band on a timer (not in
/// reply to any request). Byte counts are absolute; `cpu_pct` is the
/// busy fraction over the last sampling window, 0.0–100.0.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct AgentMetrics {
    pub cpu_pct: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub disk_used: u64,
    pub disk_total: u64,
}

/// Agent-initiated, unsolicited events. Unlike [`RpcResponse`], these are not
/// correlated to a request — the controller's reader demuxes them off the
/// shared stdout stream and routes them to observers (TUI metrics, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentEvent {
    Metrics(AgentMetrics),
}

/// Multiplexing envelope for every frame the agent writes on stdout.
///
/// The push-mode stdout stream now carries two logical channels: ordered
/// replies to controller requests ([`AgentFrame::Response`]) and out-of-band
/// agent-initiated events ([`AgentFrame::Event`]). The controller's reader
/// task decodes `AgentFrame`, forwards responses to the in-flight request and
/// events to their observers. The stdin direction stays plain [`RpcRequest`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentFrame {
    Response(RpcResponse),
    Event(AgentEvent),
}
