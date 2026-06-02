use crate::id::{MachineId, NodeId};
use crate::node::NodeStatus;
use async_trait::async_trait;
use infrazeug_shell::local::{ExecOutput, LocalShellExecutor, OutputChunk};
use infrazeug_shell::{Result as ShellResult, ShellOp};
use tokio::sync::mpsc;

#[async_trait]
pub trait OpExecutor: Send + Sync {
    async fn execute(&self, machine_id: MachineId, op: &ShellOp) -> ShellResult<ExecOutput>;

    async fn execute_streaming(
        &self,
        machine_id: MachineId,
        op: &ShellOp,
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> ShellResult<ExecOutput> {
        let out = self.execute(machine_id, op).await?;
        if let Some(tx) = output {
            if !out.stdout.is_empty() {
                let _ = tx.send(OutputChunk {
                    stream: infrazeug_shell::OutputStream::Stdout,
                    data: out.stdout.clone(),
                });
            }
            if !out.stderr.is_empty() {
                let _ = tx.send(OutputChunk {
                    stream: infrazeug_shell::OutputStream::Stderr,
                    data: out.stderr.clone(),
                });
            }
        }
        Ok(out)
    }

    async fn reconnect(&self, _machine_id: MachineId) -> ShellResult<()> {
        Ok(())
    }

    async fn sync_node_graph_state(
        &self,
        _machine_id: MachineId,
        _completed: &[(NodeId, NodeStatus)],
    ) -> ShellResult<()> {
        Ok(())
    }

    /// Register a machine discovered at apply time (dynamic-group fan-out) so this
    /// executor can connect to it lazily on first use. Default no-op for executors
    /// with a fixed machine set.
    async fn register_machine(&self, _machine: crate::machine::Machine) {}
}

pub struct LocalExecutor;

#[async_trait]
impl OpExecutor for LocalExecutor {
    async fn execute(&self, _machine_id: MachineId, op: &ShellOp) -> ShellResult<ExecOutput> {
        LocalShellExecutor::new().execute(op).await
    }

    async fn execute_streaming(
        &self,
        _machine_id: MachineId,
        op: &ShellOp,
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> ShellResult<ExecOutput> {
        LocalShellExecutor::new()
            .execute_streaming(op, output)
            .await
    }
}
