use crate::error::{Result, TransportError};
use infrazeug_rpc::{
    decode_one, encode, AgentEvent, AgentFrame, AgentMetrics, RpcNodeGraphEntry, RpcRequest,
    RpcResponse,
};
use infrazeug_shell::local::{ExecOutput, OutputChunk};
use infrazeug_shell::ShellOp;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::{mpsc, Mutex};

/// Controller-side half of the push-mode RPC microarchitecture.
///
/// Takes ownership of an agent child process's stdin/stdout pipes and
/// sends/receives length-prefixed postcard frames. Each [`RpcChannel`]
/// corresponds to one remote machine; the [`AgentPushBackend`] owns the
/// channel and exposes [`execute`](RpcChannel::execute_shell) for the
/// scheduler. The agent-side counterpart is `serve_rpc()` in
/// `infrazeug-agent/src/main.rs`.
///
/// # Demultiplexing
///
/// The agent's stdout is multiplexed (`AgentFrame`): request replies and
/// out-of-band events (metrics) share one stream. A background reader task
/// owns stdout, decodes each frame, and routes [`AgentFrame::Response`] to the
/// in-flight request over `responses` while forwarding [`AgentFrame::Event`]
/// to `metrics`. The `request_lock` still serializes whole request→reply
/// exchanges, so `responses` has exactly one consumer at a time and delivers
/// the current request's frames in order; metrics keep flowing even while a
/// long command holds the request lock.
///
/// See `docs/protocol.md` for the full microarchitecture diagram.
pub struct RpcChannel {
    stdin: Arc<Mutex<ChildStdin>>,
    /// Replies to controller requests, fed by the reader task. Guarded so the
    /// single in-flight request (holder of `request_lock`) can `recv` them.
    responses: Mutex<mpsc::UnboundedReceiver<Result<RpcResponse>>>,
    request_lock: Mutex<()>,
}

impl RpcChannel {
    /// Take ownership of the agent child's pipes and spawn the reader task.
    ///
    /// `metrics`, when provided, receives every [`AgentMetrics`] sample the
    /// agent pushes; pass `None` to discard them (non-TUI runs).
    pub fn from_child(
        child: &mut Child,
        metrics: Option<mpsc::UnboundedSender<AgentMetrics>>,
    ) -> Result<Self> {
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| TransportError::Other("agent stdin missing".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TransportError::Other("agent stdout missing".into()))?;

        let (resp_tx, resp_rx) = mpsc::unbounded_channel();
        tokio::spawn(reader_loop(stdout, resp_tx, metrics));

        Ok(Self {
            stdin: Arc::new(Mutex::new(stdin)),
            responses: Mutex::new(resp_rx),
            request_lock: Mutex::new(()),
        })
    }

    pub async fn request(&self, req: &RpcRequest) -> Result<RpcResponse> {
        let _guard = self.request_lock.lock().await;
        self.send_request(req).await?;
        self.recv_response().await
    }

    async fn send_request(&self, req: &RpcRequest) -> Result<()> {
        let frame = encode(req).map_err(|e| TransportError::Other(e.to_string()))?;
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(&frame)
            .await
            .map_err(|e| TransportError::Other(e.to_string()))?;
        stdin
            .flush()
            .await
            .map_err(|e| TransportError::Other(e.to_string()))?;
        drop(stdin);
        Ok(())
    }

    /// Receive the next reply frame for the in-flight request. Callers must
    /// hold `request_lock`, which keeps `responses` to a single consumer.
    async fn recv_response(&self) -> Result<RpcResponse> {
        match self.responses.lock().await.recv().await {
            Some(resp) => resp,
            None => Err(TransportError::Other("agent rpc reader stopped".into())),
        }
    }

    #[cfg(test)]
    pub(crate) async fn read_frame_for_test<R: tokio::io::AsyncRead + Unpin>(
        buf: &mut Vec<u8>,
        reader: &mut R,
    ) -> Result<AgentFrame> {
        read_frame(buf, reader).await
    }

    pub async fn execute_shell(&self, op: &ShellOp) -> Result<ExecOutput> {
        self.execute_shell_streaming(op, None).await
    }

    pub async fn execute_shell_streaming(
        &self,
        op: &ShellOp,
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> Result<ExecOutput> {
        let _guard = self.request_lock.lock().await;
        self.send_request(&RpcRequest::ExecuteShell { op: op.clone() })
            .await?;
        loop {
            match self.recv_response().await? {
                RpcResponse::ExecOutputChunk { stream, data } => {
                    if let Some(tx) = output.as_ref() {
                        let _ = tx.send(OutputChunk { stream, data });
                    }
                }
                RpcResponse::ExecResult(out) => return Ok(out),
                RpcResponse::Error(e) => return Err(TransportError::Other(e)),
                RpcResponse::Pong => return Err(TransportError::Other("unexpected pong".into())),
                RpcResponse::NodeGraphStateSynced => {
                    return Err(TransportError::Other(
                        "unexpected node graph sync ack".into(),
                    ))
                }
                RpcResponse::VarValue(_) | RpcResponse::VarDenied { .. } => {
                    return Err(TransportError::Other("unexpected var response".into()))
                }
                RpcResponse::NativeResult(_) => {
                    return Err(TransportError::Other("unexpected native result".into()))
                }
            }
        }
    }

    pub async fn sync_node_graph_state(&self, completed: Vec<RpcNodeGraphEntry>) -> Result<()> {
        match self
            .request(&RpcRequest::SyncNodeGraphState { completed })
            .await?
        {
            RpcResponse::NodeGraphStateSynced => Ok(()),
            RpcResponse::Error(e) => Err(TransportError::Other(e)),
            RpcResponse::Pong => Err(TransportError::Other("unexpected pong".into())),
            RpcResponse::ExecResult(_) | RpcResponse::ExecOutputChunk { .. } => {
                Err(TransportError::Other("unexpected exec response".into()))
            }
            RpcResponse::VarValue(_) | RpcResponse::VarDenied { .. } => {
                Err(TransportError::Other("unexpected var response".into()))
            }
            RpcResponse::NativeResult(_) => {
                Err(TransportError::Other("unexpected native result".into()))
            }
        }
    }

    pub async fn ping(&self) -> Result<()> {
        match self.request(&RpcRequest::Ping).await? {
            RpcResponse::Pong => Ok(()),
            RpcResponse::Error(e) => Err(TransportError::Other(e)),
            RpcResponse::ExecResult(_) => Err(TransportError::Other("unexpected exec".into())),
            RpcResponse::ExecOutputChunk { .. } => {
                Err(TransportError::Other("unexpected exec output chunk".into()))
            }
            RpcResponse::NodeGraphStateSynced => Err(TransportError::Other(
                "unexpected node graph sync ack".into(),
            )),
            RpcResponse::VarValue(_) | RpcResponse::VarDenied { .. } => {
                Err(TransportError::Other("unexpected var response".into()))
            }
            RpcResponse::NativeResult(_) => {
                Err(TransportError::Other("unexpected native result".into()))
            }
        }
    }

    pub async fn execute_native(
        &self,
        method_id: &str,
        input: &serde_cbor::Value,
    ) -> Result<infrazeug_native::NativeResult> {
        match self
            .request(&RpcRequest::ExecuteNative {
                method_id: method_id.to_string(),
                input: input.clone(),
            })
            .await?
        {
            RpcResponse::NativeResult(result) => Ok(result),
            RpcResponse::Error(e) => Err(TransportError::Other(e)),
            RpcResponse::Pong => Err(TransportError::Other("unexpected pong".into())),
            RpcResponse::ExecResult(_) | RpcResponse::ExecOutputChunk { .. } => {
                Err(TransportError::Other("unexpected exec response".into()))
            }
            RpcResponse::NodeGraphStateSynced => Err(TransportError::Other(
                "unexpected node graph sync ack".into(),
            )),
            RpcResponse::VarValue(_) | RpcResponse::VarDenied { .. } => {
                Err(TransportError::Other("unexpected var response".into()))
            }
        }
    }
}

/// Background reader: owns the agent's stdout, decodes each [`AgentFrame`], and
/// demuxes it. Responses go to the in-flight request over `resp_tx`; events are
/// forwarded to `metrics` (dropped if no observer). On stream close or decode
/// error it pushes the error to `resp_tx` so a waiting request unblocks, then
/// exits.
async fn reader_loop(
    mut stdout: ChildStdout,
    resp_tx: mpsc::UnboundedSender<Result<RpcResponse>>,
    metrics: Option<mpsc::UnboundedSender<AgentMetrics>>,
) {
    let mut buf = Vec::new();
    loop {
        match read_frame(&mut buf, &mut stdout).await {
            Ok(AgentFrame::Response(resp)) => {
                // Receiver gone => no one is waiting and never will be; stop.
                if resp_tx.send(Ok(resp)).is_err() {
                    break;
                }
            }
            Ok(AgentFrame::Event(AgentEvent::Metrics(m))) => {
                if let Some(tx) = metrics.as_ref() {
                    let _ = tx.send(m);
                }
            }
            Err(e) => {
                let _ = resp_tx.send(Err(e));
                break;
            }
        }
    }
}

/// Decode the next frame from `buf`, reading from `reader` only when `buf` does
/// not yet hold a complete frame.
///
/// A single OS read can deliver multiple frames (e.g. a streamed
/// `ExecOutputChunk` immediately followed by the final `ExecResult` for a fast
/// command, or a metrics event interleaved with either). Decoding the buffer
/// *before* reading is what prevents a trailing frame from being stranded in
/// `buf` while the next call blocks forever on a read for bytes the agent
/// already sent.
async fn read_frame<R: tokio::io::AsyncRead + Unpin>(
    buf: &mut Vec<u8>,
    reader: &mut R,
) -> Result<AgentFrame> {
    loop {
        if let Ok((frame, consumed)) = decode_one::<AgentFrame>(buf) {
            buf.drain(..consumed);
            return Ok(frame);
        }
        if buf.len() > 16 * 1024 * 1024 {
            return Err(TransportError::Other("rpc frame too large".into()));
        }

        let mut chunk = [0u8; 4096];
        let n = reader
            .read(&mut chunk)
            .await
            .map_err(|e| TransportError::Other(e.to_string()))?;
        if n == 0 {
            if buf.is_empty() {
                return Err(TransportError::Other("agent closed rpc stream".into()));
            }
            return Err(TransportError::Other(format!(
                "agent closed rpc stream with partial frame ({} bytes buffered)",
                buf.len()
            )));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_rpc::encode;
    use infrazeug_shell::local::OutputStream;
    use tokio::io::AsyncWriteExt;
    use tokio::time::{timeout, Duration};

    fn resp_frame(resp: RpcResponse) -> Vec<u8> {
        encode(&AgentFrame::Response(resp)).unwrap()
    }

    fn metrics_frame(m: AgentMetrics) -> Vec<u8> {
        encode(&AgentFrame::Event(AgentEvent::Metrics(m))).unwrap()
    }

    fn sample(cpu: f32) -> AgentMetrics {
        AgentMetrics {
            cpu_pct: cpu,
            mem_used: 1,
            mem_total: 2,
            disk_used: 3,
            disk_total: 4,
        }
    }

    // A fast command streams an output chunk and then exits; the agent writes
    // ExecOutputChunk immediately followed by ExecResult, so both frames can
    // arrive in a single read. The second read_frame must decode the buffered
    // ExecResult instead of blocking on a read that never completes (the
    // `__INFRAZEUG_UNCHANGED__` freeze).
    #[tokio::test]
    async fn second_frame_in_same_read_does_not_block() {
        let (mut writer, mut reader) = tokio::io::duplex(64 * 1024);

        let chunk = resp_frame(RpcResponse::ExecOutputChunk {
            stream: OutputStream::Stdout,
            data: b"__INFRAZEUG_UNCHANGED__\n".to_vec(),
        });
        let result = resp_frame(RpcResponse::ExecResult(ExecOutput {
            exit_code: 0,
            stdout: b"__INFRAZEUG_UNCHANGED__\n".to_vec(),
            stderr: Vec::new(),
        }));

        // Both frames in one write; writer stays open (agent now idle, as after
        // a real ExecuteShell). Old code would block on the second read here.
        let mut combined = chunk.clone();
        combined.extend_from_slice(&result);
        writer.write_all(&combined).await.unwrap();
        writer.flush().await.unwrap();

        let mut buf = Vec::new();
        let first = timeout(
            Duration::from_secs(5),
            RpcChannel::read_frame_for_test(&mut buf, &mut reader),
        )
        .await
        .expect("first frame timed out")
        .unwrap();
        assert!(matches!(
            first,
            AgentFrame::Response(RpcResponse::ExecOutputChunk { .. })
        ));

        let second = timeout(
            Duration::from_secs(5),
            RpcChannel::read_frame_for_test(&mut buf, &mut reader),
        )
        .await
        .expect("second frame deadlocked waiting on an idle agent")
        .unwrap();
        assert!(matches!(
            second,
            AgentFrame::Response(RpcResponse::ExecResult(_))
        ));
    }

    // The reader must route out-of-band metrics events to the metrics sink and
    // request replies to the response channel, even when both arrive in a
    // single read interleaved (metrics between a chunk and its result).
    #[tokio::test]
    async fn reader_demuxes_metrics_from_responses() {
        let (mut writer, reader) = tokio::io::duplex(64 * 1024);
        let (resp_tx, mut resp_rx) = mpsc::unbounded_channel();
        let (metrics_tx, mut metrics_rx) = mpsc::unbounded_channel();

        let mut bytes = metrics_frame(sample(10.0));
        bytes.extend_from_slice(&resp_frame(RpcResponse::ExecOutputChunk {
            stream: OutputStream::Stdout,
            data: b"hi\n".to_vec(),
        }));
        bytes.extend_from_slice(&metrics_frame(sample(20.0)));
        bytes.extend_from_slice(&resp_frame(RpcResponse::ExecResult(ExecOutput {
            exit_code: 0,
            stdout: Vec::new(),
            stderr: Vec::new(),
        })));
        writer.write_all(&bytes).await.unwrap();
        writer.flush().await.unwrap();
        drop(writer); // close stream so the reader terminates after draining

        // duplex reader half is AsyncRead; reader_loop wants ChildStdout, so
        // drive read_frame directly to validate the demux classification.
        tokio::spawn(async move {
            let mut r = reader;
            let mut buf = Vec::new();
            loop {
                match read_frame(&mut buf, &mut r).await {
                    Ok(AgentFrame::Response(resp)) => {
                        if resp_tx.send(Ok::<_, TransportError>(resp)).is_err() {
                            break;
                        }
                    }
                    Ok(AgentFrame::Event(AgentEvent::Metrics(m))) => {
                        let _ = metrics_tx.send(m);
                    }
                    Err(_) => break,
                }
            }
        });

        // Responses arrive in order; metrics are siphoned off separately.
        let first = timeout(Duration::from_secs(5), resp_rx.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(first, RpcResponse::ExecOutputChunk { .. }));
        let second = timeout(Duration::from_secs(5), resp_rx.recv())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(second, RpcResponse::ExecResult(_)));

        let m1 = metrics_rx.recv().await.unwrap();
        let m2 = metrics_rx.recv().await.unwrap();
        assert_eq!(m1.cpu_pct, 10.0);
        assert_eq!(m2.cpu_pct, 20.0);
    }
}
