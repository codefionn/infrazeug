//! On-target agent: `serve-rpc` (push) and `serve-pull` (pull-mode apply).
//!
//! # Push-mode RPC microarchitecture
//!
//! When invoked as `serve-rpc`, the agent reads length-prefixed postcard
//! frames from stdin and writes framed responses to stdout. The controller
//! (running on the build machine) connects over SSH, uploads this binary,
//! and drives it via [`RpcRequest`]/[`RpcResponse`] messages. This is the
//! push-mode half of the planning protocol; see `docs/protocol.md`.
//!
//! The pull-mode path (`serve-pull`) does not use postcard RPC — it reads
//! a sealed CBOR `PlanSlice` from the local `PlanStore` instead.
//!
//! # Multiplexed stdout
//!
//! Stdout carries an [`AgentFrame`] stream: request replies
//! ([`AgentFrame::Response`]) and out-of-band events the agent emits on its own
//! timer ([`AgentFrame::Event`], currently [`AgentMetrics`]). A shared
//! [`FrameWriter`] serializes whole frames so the request handler and the
//! background metrics task never interleave mid-frame on the pipe.

mod metrics;

use infrazeug_api::pull_cli::PullCommandSet;
use infrazeug_api::PlaybookBundle;
use infrazeug_api::{init_tracing, run, ExtraSubcommand, RunCommands, RunConfig};
use infrazeug_core::Infra;
use infrazeug_native::{builtin_registry, MethodRegistry, NodeCtx};
use infrazeug_rpc::frame::{decode_one, encode};
use infrazeug_rpc::{AgentEvent, AgentFrame, RpcNodeStatus, RpcRequest, RpcResponse};
use infrazeug_shell::local::{LocalShellExecutor, OutputChunk};
use infrazeug_shell::ShellOp;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, Stdout};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

/// How often the agent samples and pushes resource metrics.
const METRICS_INTERVAL: Duration = Duration::from_secs(2);

/// Serializes frame writes to the agent's stdout. Cloneable so the request
/// loop and the metrics task can share one stdout without interleaving frames:
/// each `write_*` encodes a whole frame, then writes it under the lock.
#[derive(Clone)]
struct FrameWriter(Arc<Mutex<Stdout>>);

impl FrameWriter {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(tokio::io::stdout())))
    }

    async fn write_frame(&self, frame: &AgentFrame) -> std::io::Result<()> {
        let bytes = encode(frame).map_err(std::io::Error::other)?;
        let mut out = self.0.lock().await;
        out.write_all(&bytes).await?;
        out.flush().await
    }

    async fn write_response(&self, resp: RpcResponse) -> std::io::Result<()> {
        self.write_frame(&AgentFrame::Response(resp)).await
    }

    async fn write_event(&self, event: AgentEvent) -> std::io::Result<()> {
        self.write_frame(&AgentFrame::Event(event)).await
    }
}

/// Sample resource usage on a timer and push it as an out-of-band event.
/// Collection errors (e.g. non-Linux, unreadable `/proc`) are skipped; a write
/// error means the controller is gone, so the task exits.
async fn metrics_loop(writer: FrameWriter) {
    let mut sampler = metrics::CpuSampler::new();
    let mut tick = tokio::time::interval(METRICS_INTERVAL);
    loop {
        tick.tick().await;
        if let Ok(sample) = metrics::collect(&mut sampler).await {
            if writer
                .write_event(AgentEvent::Metrics(sample))
                .await
                .is_err()
            {
                break;
            }
        }
    }
}

fn agent_native_registry() -> &'static MethodRegistry {
    static REG: std::sync::OnceLock<MethodRegistry> = std::sync::OnceLock::new();
    REG.get_or_init(builtin_registry)
}

static EXTRAS: [ExtraSubcommand; 1] = [ExtraSubcommand {
    name: "serve-rpc",
    about: "Postcard RPC over stdio (push-mode agent path)",
    run: || Box::pin(serve_rpc()),
}];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("infrazeug-agent")
            .about("On-host agent (serve-rpc, serve-pull)")
            .commands(RunCommands::empty())
            .pull(PullCommandSet::empty().with(PullCommandSet::SERVE_PULL))
            .extras(&EXTRAS),
        |_| Ok(PlaybookBundle::from_infra(Infra::new())),
    )
    .await
}

/// Push-mode RPC event loop: read framed `RpcRequest`s from stdin,
/// dispatch to [`handle_request`], write framed `RpcResponse`s to stdout.
///
/// This is the core of the push-mode agent microarchitecture. The
/// controller's [`RpcChannel`] (`infrazeug-transport/src/ssh/rpc_channel.rs`)
/// is the counterpart that writes to this process's stdin pipe.
async fn serve_rpc() -> anyhow::Result<()> {
    let executor = LocalShellExecutor::new();
    let mut node_graph_state = HashMap::new();
    let mut stdin = tokio::io::stdin();
    let writer = FrameWriter::new();
    let metrics_task = tokio::spawn(metrics_loop(writer.clone()));
    let mut buf = Vec::new();
    let mut scratch = [0u8; 4096];

    loop {
        let n = stdin.read(&mut scratch).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&scratch[..n]);

        loop {
            match decode_one::<RpcRequest>(&buf) {
                Ok((req, consumed)) => {
                    buf.drain(..consumed);
                    let resp = handle_request(&executor, req, &writer, &mut node_graph_state).await;
                    writer.write_response(resp).await?;
                }
                Err(infrazeug_rpc::FrameError::Eof) => break,
                Err(e) => {
                    writer
                        .write_response(RpcResponse::Error(e.to_string()))
                        .await?;
                    buf.clear();
                }
            }
        }
    }
    metrics_task.abort();
    Ok(())
}

async fn handle_request(
    executor: &LocalShellExecutor,
    req: RpcRequest,
    writer: &FrameWriter,
    node_graph_state: &mut HashMap<Uuid, RpcNodeStatus>,
) -> RpcResponse {
    match req {
        RpcRequest::Ping => RpcResponse::Pong,
        RpcRequest::ExecuteShell { op } => match execute_op(executor, &op, writer).await {
            Ok(out) => RpcResponse::ExecResult(out),
            Err(e) => RpcResponse::Error(e),
        },
        RpcRequest::SyncNodeGraphState { completed } => {
            for entry in completed {
                node_graph_state.insert(entry.node_id, entry.status);
            }
            RpcResponse::NodeGraphStateSynced
        }
        RpcRequest::VarRequest { .. } => RpcResponse::VarDenied {
            reason: "controller must serve vars (agent-only path)".into(),
        },
        RpcRequest::ExecuteNative { method_id, input } => {
            // Agent-side execution has no controller vault; secrets stay `None`.
            let ctx = NodeCtx::new(Uuid::nil(), Uuid::nil());
            match agent_native_registry()
                .execute(&ctx, &method_id, input)
                .await
            {
                Ok(result) => RpcResponse::NativeResult(result),
                Err(e) => RpcResponse::Error(e.to_string()),
            }
        }
    }
}

async fn execute_op(
    executor: &LocalShellExecutor,
    op: &ShellOp,
    writer: &FrameWriter,
) -> Result<infrazeug_shell::local::ExecOutput, String> {
    if matches!(op, ShellOp::SyncDir { .. }) {
        return Err(
            "SyncDir must be executed by the controller transport, not the remote agent".into(),
        );
    }
    let (tx, mut rx) = mpsc::unbounded_channel::<OutputChunk>();
    let fut = executor.execute_streaming(op, Some(tx));
    tokio::pin!(fut);
    let mut rx_open = true;

    loop {
        tokio::select! {
            maybe = rx.recv(), if rx_open => {
                match maybe {
                    Some(chunk) => write_output_chunk(writer, chunk).await?,
                    None => rx_open = false,
                }
            }
            result = &mut fut => {
                while let Ok(chunk) = rx.try_recv() {
                    write_output_chunk(writer, chunk).await?;
                }
                return result.map_err(|e| e.to_string());
            }
        }
    }
}

async fn write_output_chunk(writer: &FrameWriter, chunk: OutputChunk) -> Result<(), String> {
    writer
        .write_response(RpcResponse::ExecOutputChunk {
            stream: chunk.stream,
            data: chunk.data,
        })
        .await
        .map_err(|e| e.to_string())
}
