use super::rpc_channel::RpcChannel;
use super::session::SshSession;
use crate::error::{Result, TransportError};
use infrazeug_core::id::NodeId;
use infrazeug_core::node::NodeStatus;
use infrazeug_rpc::{AgentMetrics, RpcNodeGraphEntry, RpcNodeStatus};
use infrazeug_shell::local::{ExecOutput, OutputChunk};
use infrazeug_shell::{plan_sync_dir, FileSource, Result as ShellResult, ShellError, ShellOp};
use infrazeug_shell::{SyncDirEntry, SyncDirOptions};
use std::path::Path;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

const AGENT_REMOTE_DIR: &str = ".cache/infrazeug";
const AGENT_VERSION: &str = "0.1.0";

/// Push-mode transport backend: SCPs the agent binary to the remote host,
/// starts `serve-rpc` over SSH, and drives execution via [`RpcChannel`].
///
/// This is the push-mode entry point in the transport microarchitecture.
/// The lifecycle is: upload → spawn → ping handshake → steady-state RPC.
/// See `docs/protocol.md`.
pub struct AgentPushBackend {
    session: SshSession,
    rpc: RpcChannel,
}

impl AgentPushBackend {
    /// Connect and start `serve-rpc`. `metrics`, when set, receives the agent's
    /// out-of-band [`AgentMetrics`] samples for the lifetime of the channel.
    pub async fn connect(
        session: SshSession,
        agent_local: impl AsRef<Path>,
        metrics: Option<mpsc::UnboundedSender<AgentMetrics>>,
    ) -> Result<Self> {
        let agent_local = agent_local.as_ref().to_path_buf();
        if !agent_local.is_file() {
            return Err(TransportError::Other(format!(
                "agent binary not found at {}",
                agent_local.display()
            )));
        }

        let dest = session.destination();
        let remote_name = format!("agent-{AGENT_VERSION}");
        let home = session.remote_home().await?;
        let remote_bin = format!("{home}/{AGENT_REMOTE_DIR}/{remote_name}");

        let len = tokio::fs::metadata(&agent_local)
            .await
            .map_err(|e| TransportError::Other(e.to_string()))?
            .len();
        tracing::debug!(%dest, %remote_bin, bytes = len, "pushing infrazeug-agent");
        let data = tokio::fs::read(&agent_local)
            .await
            .map_err(|e| TransportError::Other(e.to_string()))?;
        session.upload_bytes(&remote_bin, &data, 0o755).await?;

        tracing::debug!(%dest, "starting agent serve-rpc");
        let mut cmd = Command::new("ssh");
        for a in session.base_ssh_args() {
            cmd.arg(a);
        }
        cmd.arg("-T");
        cmd.arg(&dest);
        cmd.arg("--");
        cmd.arg(format!("{remote_bin} serve-rpc"));
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| TransportError::Other(e.to_string()))?;
        let rpc = RpcChannel::from_child(&mut child, metrics)?;
        let backend = Self { session, rpc };
        timeout(Duration::from_secs(30), backend.rpc.ping())
            .await
            .map_err(|_| TransportError::Other("agent ping timed out".into()))??;
        tracing::debug!(%dest, "agent rpc ready");
        Ok(backend)
    }

    pub async fn execute(&self, op: &ShellOp) -> ShellResult<ExecOutput> {
        self.execute_streaming(op, None).await
    }

    pub async fn execute_streaming(
        &self,
        op: &ShellOp,
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> ShellResult<ExecOutput> {
        if let ShellOp::Seq { steps } = op {
            if steps.iter().any(contains_sync_dir) {
                return self.execute_seq(steps, output).await;
            }
        }
        if let ShellOp::SyncDir { src, dest, options } = op {
            return self.sync_dir(src, dest, options, output).await;
        }
        self.rpc
            .execute_shell_streaming(op, output)
            .await
            .map_err(|e| ShellError::Other(e.to_string()))
    }

    pub async fn execute_native(
        &self,
        method_id: &str,
        input: &serde_cbor::Value,
    ) -> ShellResult<infrazeug_native::NativeResult> {
        self.rpc
            .execute_native(method_id, input)
            .await
            .map_err(|e| ShellError::Other(e.to_string()))
    }

    pub async fn sync_node_graph_state(
        &self,
        completed: &[(NodeId, NodeStatus)],
    ) -> ShellResult<()> {
        let completed = completed
            .iter()
            .map(|(node_id, status)| RpcNodeGraphEntry {
                node_id: node_id.0,
                status: rpc_node_status(*status),
            })
            .collect();
        self.rpc
            .sync_node_graph_state(completed)
            .await
            .map_err(|e| ShellError::Other(e.to_string()))
    }

    #[allow(dead_code)]
    pub fn session(&self) -> &SshSession {
        &self.session
    }

    async fn execute_seq(
        &self,
        steps: &[ShellOp],
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> ShellResult<ExecOutput> {
        let mut last = ExecOutput {
            exit_code: 0,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        for step in steps {
            last = Box::pin(self.execute_streaming(step, output.clone())).await?;
            if last.exit_code != 0 {
                return Ok(last);
            }
        }
        Ok(last)
    }

    /// Lower SyncDir to a single `Seq` of agent-executable ops so the whole
    /// tree transfers in one RPC roundtrip instead of one per entry.
    async fn sync_dir(
        &self,
        src: &Path,
        dest: &Path,
        options: &SyncDirOptions,
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> ShellResult<ExecOutput> {
        let plan = plan_sync_dir(src, options)?;
        let mut steps = Vec::with_capacity(plan.entries.len() + 2);
        if options.delete {
            steps.push(ShellOp::run(vec![
                "rm".into(),
                "-rf".into(),
                dest.display().to_string(),
            ]));
        }
        steps.push(ShellOp::EnsureDir {
            path: dest.to_path_buf(),
            mode: 0o755,
        });

        for entry in &plan.entries {
            let op = match entry {
                SyncDirEntry::Dir { rel, mode } => ShellOp::EnsureDir {
                    path: dest.join(rel),
                    mode: *mode,
                },
                SyncDirEntry::File {
                    rel,
                    mode,
                    hard_link_to,
                } => {
                    let path = dest.join(rel);
                    if let Some(link_to) = hard_link_to {
                        ShellOp::run(vec![
                            "ln".into(),
                            "-f".into(),
                            dest.join(link_to).display().to_string(),
                            path.display().to_string(),
                        ])
                    } else {
                        let data = tokio::fs::read(src.join(rel)).await?;
                        ShellOp::WriteFile {
                            path,
                            content: FileSource::bytes(data),
                            mode: *mode,
                        }
                    }
                }
                SyncDirEntry::Symlink { rel, target } => {
                    let path = dest.join(rel);
                    ShellOp::run(vec![
                        "sh".into(),
                        "-c".into(),
                        "rm -rf \"$2\" && ln -s \"$1\" \"$2\"".into(),
                        "infrazeug-sync-link".into(),
                        target.display().to_string(),
                        path.display().to_string(),
                    ])
                }
            };
            steps.push(op);
        }

        let out = self
            .rpc
            .execute_shell_streaming(&ShellOp::Seq { steps }, output)
            .await
            .map_err(|e| ShellError::Other(e.to_string()))?;
        if out.exit_code != 0 {
            return Ok(out);
        }

        Ok(ExecOutput {
            exit_code: 0,
            stdout: format!("synced {} entries\n", plan.entries.len()).into_bytes(),
            stderr: Vec::new(),
        })
    }
}

fn contains_sync_dir(op: &ShellOp) -> bool {
    match op {
        ShellOp::SyncDir { .. } => true,
        ShellOp::Seq { steps } => steps.iter().any(contains_sync_dir),
        _ => false,
    }
}

fn rpc_node_status(status: NodeStatus) -> RpcNodeStatus {
    match status {
        NodeStatus::Pending => RpcNodeStatus::Pending,
        NodeStatus::Running => RpcNodeStatus::Running,
        NodeStatus::Changed => RpcNodeStatus::Changed,
        NodeStatus::Unchanged => RpcNodeStatus::Unchanged,
        NodeStatus::Skipped => RpcNodeStatus::Skipped,
        NodeStatus::Failed => RpcNodeStatus::Failed,
        NodeStatus::Cancelled => RpcNodeStatus::Cancelled,
    }
}
