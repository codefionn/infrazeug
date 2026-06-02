use crate::error::{Result, TransportError};
use crate::ssh::{AgentPushBackend, AgentlessBackend, SshAuthResolver, SshSession};
use async_trait::async_trait;
use futures::future::join_all;
use infrazeug_core::error::CoreError;
use infrazeug_core::events::{MachineMetrics, MachinePreparePhase, SchedEvent};
use infrazeug_core::exec::OpExecutor;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::machine::{Machine, MachineKind, OsHint, SshConfig};
use infrazeug_core::native_exec::NativeExecutor;
use infrazeug_core::node::NodeStatus;
use infrazeug_core::transport::TransportChoice;
use infrazeug_core::Infra;
use infrazeug_emulate_oci::PodmanExec;
use infrazeug_shell::local::{ExecOutput, LocalShellExecutor, OutputChunk};
use infrazeug_shell::{Result as ShellResult, ShellError, ShellOp};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

enum Backend {
    Local(LocalShellExecutor),
    Container(PodmanExec),
    Agentless(AgentlessBackend),
    AgentPush(AgentPushBackend),
}

#[derive(Clone)]
struct MachineConnectInfo {
    ssh: Option<SshConfig>,
    transport: TransportChoice,
    triple: Option<String>,
}

pub struct TransportFactory {
    backends: Mutex<HashMap<MachineId, Arc<Backend>>>,
    connect_info: Mutex<HashMap<MachineId, MachineConnectInfo>>,
    infra: Mutex<Option<Infra>>,
    run_dir: PathBuf,
    agent_workspace: PathBuf,
    machine_triples: Mutex<HashMap<MachineId, String>>,
    container_exec: Mutex<HashMap<MachineId, PodmanExec>>,
    prepare_events: Mutex<Option<broadcast::Sender<SchedEvent>>>,
    /// Resolves interactive SSH auth secrets (prompt / vault) into askpass files
    /// on demand, including for lazy and dynamically-discovered machines. `None`
    /// on non-interactive runs.
    ssh_resolver: Mutex<Option<Arc<dyn SshAuthResolver>>>,
    /// Cargo profile for on-demand agent builds (mirrors `INFRAZEUG_RELEASE`).
    release: bool,
    /// Per-triple build guards so concurrent connect nodes of the same triple
    /// cross-compile the agent once.
    build_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl TransportFactory {
    pub fn new(run_dir: PathBuf, agent_workspace: PathBuf, release: bool) -> Arc<Self> {
        Arc::new(Self {
            backends: Mutex::new(HashMap::new()),
            connect_info: Mutex::new(HashMap::new()),
            infra: Mutex::new(None),
            run_dir,
            agent_workspace,
            machine_triples: Mutex::new(HashMap::new()),
            container_exec: Mutex::new(HashMap::new()),
            prepare_events: Mutex::new(None),
            ssh_resolver: Mutex::new(None),
            release,
            build_locks: Mutex::new(HashMap::new()),
        })
    }

    /// Install the interactive SSH auth resolver. Call before
    /// [`prepare`](Self::prepare) so the first connection (and any arch probe)
    /// can authenticate; the same resolver also serves lazy / discovered
    /// machines that connect later in the run.
    pub async fn set_ssh_resolver(&self, resolver: Option<Arc<dyn SshAuthResolver>>) {
        *self.ssh_resolver.lock().await = resolver;
    }

    /// Resolve the askpass secret file for `machine_id`'s SSH auth, prompting or
    /// reading the vault on first use (cached by the resolver). `Ok(None)` for
    /// non-interactive machines; errors if interactive auth is configured but no
    /// resolver is wired for this run.
    async fn askpass_for(&self, machine_id: MachineId, ssh: &SshConfig) -> Result<Option<PathBuf>> {
        if !ssh.auth.is_interactive() {
            return Ok(None);
        }
        let resolver = self.ssh_resolver.lock().await.clone();
        match resolver {
            Some(r) => r
                .askpass_file(machine_id, ssh)
                .await
                .map_err(TransportError::Other),
            None => Err(TransportError::Other(format!(
                "machine {machine_id} requires interactive SSH auth, but no auth resolver is configured for this run"
            ))),
        }
    }

    /// Resolve the agent binary path for `triple`, cross-compiling it on demand if
    /// it is not already present. The per-triple [`build_locks`](Self::build_locks)
    /// guard makes concurrent connect nodes for the same triple build exactly once.
    async fn ensure_agent_built(&self, machine_id: MachineId, triple: &str) -> Result<PathBuf> {
        let path = self.agent_path_for_triple(triple);
        if tokio::fs::metadata(&path).await.is_ok() {
            return Ok(path);
        }
        let lock = {
            let mut locks = self.build_locks.lock().await;
            locks
                .entry(triple.to_string())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = lock.lock().await;
        // A sibling connect node may have finished the build while we waited.
        if tokio::fs::metadata(&path).await.is_ok() {
            return Ok(path);
        }
        self.emit_prepare_machine(
            machine_id,
            MachinePreparePhase::BuildingAgent,
            Some(triple.to_string()),
        )
        .await;
        let workspace = self.agent_workspace.clone();
        let triple_owned = triple.to_string();
        let release = self.release;
        tokio::task::spawn_blocking(move || {
            infrazeug_build::build_agent(
                &workspace,
                &infrazeug_build::AgentBuildOptions {
                    targets: vec![triple_owned],
                    release,
                    quiet: true,
                },
            )
        })
        .await
        .map_err(|e| TransportError::Other(format!("agent build task failed: {e}")))?
        .map_err(TransportError::Other)?;
        Ok(path)
    }

    pub async fn set_prepare_events(&self, events: Option<broadcast::Sender<SchedEvent>>) {
        *self.prepare_events.lock().await = events;
    }

    async fn emit_prepare_machine(
        &self,
        machine_id: MachineId,
        phase: MachinePreparePhase,
        detail: Option<String>,
    ) {
        if let Some(tx) = self.prepare_events.lock().await.as_ref() {
            let _ = tx.send(SchedEvent::PrepareMachine {
                machine: machine_id,
                phase,
                detail,
            });
        }
    }

    /// Build a sink for a push-mode agent's out-of-band metrics, forwarding each
    /// sample to the event stream as [`SchedEvent::MachineMetrics`]. Returns
    /// `None` when no event sink is wired (e.g. non-TUI runs), so the agent's
    /// metrics are simply dropped by the reader. The forwarder task ends when
    /// the channel closes (agent disconnect / backend drop), so it is
    /// self-cleaning across reconnects.
    async fn agent_metrics_sink(
        &self,
        machine_id: MachineId,
    ) -> Option<mpsc::UnboundedSender<infrazeug_rpc::AgentMetrics>> {
        let events = self.prepare_events.lock().await.clone()?;
        let (tx, mut rx) = mpsc::unbounded_channel::<infrazeug_rpc::AgentMetrics>();
        tokio::spawn(async move {
            while let Some(m) = rx.recv().await {
                let _ = events.send(SchedEvent::MachineMetrics {
                    machine: machine_id,
                    metrics: MachineMetrics {
                        cpu_pct: m.cpu_pct,
                        mem_used: m.mem_used,
                        mem_total: m.mem_total,
                        disk_used: m.disk_used,
                        disk_total: m.disk_total,
                    },
                });
            }
        });
        Some(tx)
    }

    pub async fn set_machine_triples(&self, triples: HashMap<MachineId, String>) {
        *self.machine_triples.lock().await = triples;
    }

    fn agent_path_for_triple(&self, triple: &str) -> PathBuf {
        infrazeug_build::agent_path_for_triple(&self.agent_workspace, triple)
    }

    /// Resolve a push machine's target triple, caching the result. Prefers a
    /// pre-seeded triple (e.g. from a prior probe), then a declared `uname -m`
    /// arch hint, and finally probes the host over SSH. This keeps agent build
    /// self-sufficient now that there is no eager pre-apply probe phase.
    async fn resolve_triple(
        &self,
        machine_id: MachineId,
        ssh: &SshConfig,
        os: Option<&OsHint>,
    ) -> Result<String> {
        if let Some(t) = self.machine_triples.lock().await.get(&machine_id).cloned() {
            return Ok(t);
        }
        let triple = if let Some(arch) = os.and_then(|h| h.arch.clone()) {
            infrazeug_build::uname_machine_to_triple(&arch)
        } else {
            self.emit_prepare_machine(machine_id, MachinePreparePhase::ProbingArch, None)
                .await;
            let askpass = self.askpass_for(machine_id, ssh).await?;
            let uname = crate::ssh::probe_uname_machine(ssh, askpass.as_deref()).await?;
            infrazeug_build::uname_machine_to_triple(&uname)
        };
        self.machine_triples
            .lock()
            .await
            .insert(machine_id, triple.clone());
        Ok(triple)
    }

    pub async fn register_container(self: &Arc<Self>, machine_id: MachineId, exec: PodmanExec) {
        self.container_exec.lock().await.insert(machine_id, exec);
    }

    async fn backend_for(&self, infra: &Infra, machine_id: MachineId) -> Result<()> {
        {
            let map = self.backends.lock().await;
            if map.contains_key(&machine_id) {
                return Ok(());
            }
        }
        let machine = infra
            .machine_by_id(machine_id)
            .ok_or_else(|| TransportError::Other(format!("unknown machine {machine_id}")))?;
        let choice = infra.transport_for_machine(machine);
        let backend = match self.build_backend(infra, machine, choice).await {
            Ok(b) => b,
            Err(e) => {
                self.emit_prepare_machine(
                    machine_id,
                    MachinePreparePhase::Failed {
                        message: e.to_string(),
                    },
                    None,
                )
                .await;
                return Err(e);
            }
        };
        self.emit_prepare_machine(machine_id, MachinePreparePhase::Ready, None)
            .await;
        self.save_connect_info(machine, choice).await;
        {
            let mut map = self.backends.lock().await;
            map.insert(machine_id, Arc::new(backend));
        }
        Ok(())
    }

    async fn save_connect_info(&self, machine: &Machine, transport: TransportChoice) {
        let ssh = match &machine.kind {
            MachineKind::Remote { ssh, .. } => Some(ssh.clone()),
            _ => None,
        };
        let triple = self.machine_triples.lock().await.get(&machine.id).cloned();
        self.connect_info.lock().await.insert(
            machine.id,
            MachineConnectInfo {
                ssh,
                transport,
                triple,
            },
        );
    }

    async fn build_backend(
        &self,
        _infra: &Infra,
        machine: &Machine,
        choice: TransportChoice,
    ) -> Result<Backend> {
        if let MachineKind::Container(_) = &machine.kind {
            let map = self.container_exec.lock().await;
            if let Some(ex) = map.get(&machine.id) {
                self.emit_prepare_machine(
                    machine.id,
                    MachinePreparePhase::Connecting,
                    Some("container exec".into()),
                )
                .await;
                return Ok(Backend::Container(ex.clone()));
            }
            return Err(TransportError::Other(format!(
                "container machine {} has no runtime (build/start first)",
                machine.name
            )));
        }

        match (choice, &machine.kind) {
            (TransportChoice::Local, MachineKind::Local) => {
                Ok(Backend::Local(LocalShellExecutor::new()))
            }
            (TransportChoice::SshAgentless, MachineKind::Remote { ssh, .. }) => {
                self.emit_prepare_machine(
                    machine.id,
                    MachinePreparePhase::Connecting,
                    Some("ssh agentless".into()),
                )
                .await;
                SshSession::check_openssh().await?;
                let askpass = self.askpass_for(machine.id, ssh).await?;
                let session =
                    SshSession::new(ssh.clone(), &self.run_dir).with_askpass_file(askpass);
                Ok(Backend::Agentless(AgentlessBackend::new(session)))
            }
            (TransportChoice::SshAgentPush, MachineKind::Remote { ssh, os }) => {
                SshSession::check_openssh().await?;
                let askpass = self.askpass_for(machine.id, ssh).await?;
                let session =
                    SshSession::new(ssh.clone(), &self.run_dir).with_askpass_file(askpass);
                let triple = self.resolve_triple(machine.id, ssh, os.as_ref()).await?;
                let agent_path = self.ensure_agent_built(machine.id, &triple).await?;
                let agent_bytes = tokio::fs::metadata(&agent_path).await.ok().map(|m| m.len());
                self.emit_prepare_machine(
                    machine.id,
                    MachinePreparePhase::UploadingAgent,
                    agent_bytes.map(|n| format!("{n} bytes · {triple}")),
                )
                .await;
                self.emit_prepare_machine(
                    machine.id,
                    MachinePreparePhase::Connecting,
                    Some("serve-rpc".into()),
                )
                .await;
                let metrics = self.agent_metrics_sink(machine.id).await;
                let push = AgentPushBackend::connect(session, &agent_path, metrics).await?;
                Ok(Backend::AgentPush(push))
            }
            (TransportChoice::Local, MachineKind::Remote { .. }) => Err(TransportError::Other(
                "remote machine cannot use Local transport".into(),
            )),
            (_, MachineKind::Local) => Ok(Backend::Local(LocalShellExecutor::new())),
            (TransportChoice::PullDaemon, _) => Err(TransportError::Other(
                "PullDaemon transport is not implemented (M6)".into(),
            )),
            (_, MachineKind::Container(_)) => unreachable!("container handled above"),
        }
    }

    async fn rebuild_backend(&self, machine_id: MachineId) -> Result<()> {
        let info = {
            let info_map = self.connect_info.lock().await;
            info_map.get(&machine_id).cloned()
        };
        let Some(info) = info else {
            return Err(TransportError::Other(format!(
                "no connect info for machine {machine_id}"
            )));
        };

        let backend = match (&info.ssh, &info.transport) {
            (Some(ssh), TransportChoice::SshAgentless) => {
                let askpass = self.askpass_for(machine_id, ssh).await?;
                let session =
                    SshSession::new(ssh.clone(), &self.run_dir).with_askpass_file(askpass);
                Backend::Agentless(AgentlessBackend::new(session))
            }
            (Some(ssh), TransportChoice::SshAgentPush) => {
                let askpass = self.askpass_for(machine_id, ssh).await?;
                let session =
                    SshSession::new(ssh.clone(), &self.run_dir).with_askpass_file(askpass);
                let triple = info.triple.clone().unwrap_or_else(|| {
                    infrazeug_build::host_triple().unwrap_or_else(|| "host".to_string())
                });
                let agent_path = self.ensure_agent_built(machine_id, &triple).await?;
                let metrics = self.agent_metrics_sink(machine_id).await;
                let push = AgentPushBackend::connect(session, &agent_path, metrics).await?;
                Backend::AgentPush(push)
            }
            (None, _) => Backend::Local(LocalShellExecutor::new()),
            _ => {
                return Err(TransportError::Other(format!(
                    "cannot reconnect machine {machine_id}: unsupported transport {:?}",
                    info.transport
                )))
            }
        };

        self.backends
            .lock()
            .await
            .insert(machine_id, Arc::new(backend));
        tracing::debug!(%machine_id, "transport reconnected");
        Ok(())
    }

    async fn backend_handle(&self, machine_id: MachineId) -> Option<Arc<Backend>> {
        self.backends.lock().await.get(&machine_id).cloned()
    }

    async fn require_backend_handle(&self, machine_id: MachineId) -> ShellResult<Arc<Backend>> {
        self.backend_handle(machine_id).await.ok_or_else(|| {
            ShellError::Other(format!(
                "transport not initialized for machine {machine_id}"
            ))
        })
    }
}

#[async_trait]
impl OpExecutor for TransportFactory {
    async fn execute(&self, machine_id: MachineId, op: &ShellOp) -> ShellResult<ExecOutput> {
        if let Some(backend) = self.backend_handle(machine_id).await {
            return execute_backend(backend.as_ref(), op).await;
        }
        let infra = self.infra.lock().await.clone().ok_or_else(|| {
            ShellError::Other(format!(
                "transport not initialized for machine {machine_id} (no infra)"
            ))
        })?;
        tracing::debug!(%machine_id, "lazy transport init");
        self.lazy_prepare_backend(&infra, machine_id)
            .await
            .map_err(|e| ShellError::Other(e.to_string()))?;
        let backend = self.require_backend_handle(machine_id).await?;
        execute_backend(backend.as_ref(), op).await
    }

    async fn reconnect(&self, machine_id: MachineId) -> ShellResult<()> {
        tracing::debug!(%machine_id, "reconnecting transport");
        self.rebuild_backend(machine_id)
            .await
            .map_err(|e| ShellError::Other(e.to_string()))
    }

    async fn sync_node_graph_state(
        &self,
        machine_id: MachineId,
        completed: &[(NodeId, NodeStatus)],
    ) -> ShellResult<()> {
        let Some(backend) = self.backend_handle(machine_id).await else {
            return Ok(());
        };
        match backend.as_ref() {
            Backend::AgentPush(ex) => ex.sync_node_graph_state(completed).await,
            Backend::Local(_) | Backend::Container(_) | Backend::Agentless(_) => Ok(()),
        }
    }

    async fn register_machine(&self, machine: Machine) {
        // Add to the factory's infra clone so `lazy_prepare_backend` can resolve
        // the discovered machine when its first node executes.
        let mut guard = self.infra.lock().await;
        if let Some(infra) = guard.as_mut() {
            if !infra.machines.iter().any(|m| m.id == machine.id) {
                infra.machines.push(machine);
            }
        }
    }

    async fn execute_streaming(
        &self,
        machine_id: MachineId,
        op: &ShellOp,
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> ShellResult<ExecOutput> {
        if let Some(backend) = self.backend_handle(machine_id).await {
            return execute_backend_streaming(backend.as_ref(), op, output).await;
        }
        let infra = self.infra.lock().await.clone().ok_or_else(|| {
            ShellError::Other(format!(
                "transport not initialized for machine {machine_id} (no infra)"
            ))
        })?;
        tracing::debug!(%machine_id, "lazy transport init");
        self.lazy_prepare_backend(&infra, machine_id)
            .await
            .map_err(|e| ShellError::Other(e.to_string()))?;
        let backend = self.require_backend_handle(machine_id).await?;
        execute_backend_streaming(backend.as_ref(), op, output).await
    }
}

#[async_trait]
impl NativeExecutor for TransportFactory {
    async fn execute_native(
        &self,
        machine_id: MachineId,
        _node_id: NodeId,
        method_id: &str,
        input: &serde_cbor::Value,
        // Remote agents resolve their own secrets; the controller vault never
        // crosses the RPC boundary.
        _secrets: Option<std::sync::Arc<dyn infrazeug_native::SecretSource>>,
    ) -> infrazeug_core::Result<infrazeug_native::NativeResult> {
        if let Some(backend) = self.backend_handle(machine_id).await {
            return execute_native_backend(backend.as_ref(), method_id, input).await;
        }
        let infra = self.infra.lock().await.clone().ok_or_else(|| {
            CoreError::other(format!(
                "transport not initialized for machine {machine_id} (no infra)"
            ))
        })?;
        self.lazy_prepare_backend(&infra, machine_id)
            .await
            .map_err(|e| CoreError::other(e.to_string()))?;
        let backend = self.backend_handle(machine_id).await.ok_or_else(|| {
            CoreError::other(format!(
                "transport not initialized for machine {machine_id}"
            ))
        })?;
        execute_native_backend(backend.as_ref(), method_id, input).await
    }
}

async fn execute_native_backend(
    backend: &Backend,
    method_id: &str,
    input: &serde_cbor::Value,
) -> infrazeug_core::Result<infrazeug_native::NativeResult> {
    match backend {
        Backend::AgentPush(ex) => ex
            .execute_native(method_id, input)
            .await
            .map_err(|e| CoreError::other(e.to_string())),
        Backend::Local(_) | Backend::Container(_) | Backend::Agentless(_) => Err(CoreError::other(
            "native RPC requires push-mode agent transport",
        )),
    }
}

async fn execute_backend(backend: &Backend, op: &ShellOp) -> ShellResult<ExecOutput> {
    match backend {
        Backend::Local(ex) => ex.execute(op).await,
        Backend::Container(ex) => ex.execute(op).await,
        Backend::Agentless(ex) => ex.execute(op).await,
        Backend::AgentPush(ex) => ex.execute(op).await,
    }
}

async fn execute_backend_streaming(
    backend: &Backend,
    op: &ShellOp,
    output: Option<mpsc::UnboundedSender<OutputChunk>>,
) -> ShellResult<ExecOutput> {
    match backend {
        Backend::Local(ex) => ex.execute_streaming(op, output).await,
        Backend::Container(ex) => {
            let out = ex.execute(op).await?;
            emit_buffered_output(&out, output);
            Ok(out)
        }
        Backend::Agentless(ex) => ex.execute_streaming(op, output).await,
        Backend::AgentPush(ex) => ex.execute_streaming(op, output).await,
    }
}

fn emit_buffered_output(out: &ExecOutput, output: Option<mpsc::UnboundedSender<OutputChunk>>) {
    let Some(tx) = output else {
        return;
    };
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

impl TransportFactory {
    pub async fn prepare(&self, infra: &Infra) -> Result<()> {
        *self.infra.lock().await = Some(infra.clone());
        let futs: Vec<_> = infra
            .machines
            .iter()
            .filter(|m| !m.lazy)
            .map(|m| async move { self.backend_for(infra, m.id).await })
            .collect();
        let results = join_all(futs).await;
        for r in results {
            r?;
        }
        for m in infra.machines.iter().filter(|m| m.lazy) {
            self.emit_prepare_machine(
                m.id,
                MachinePreparePhase::Skipped {
                    reason: "lazy (connect on first use)".into(),
                },
                None,
            )
            .await;
        }
        Ok(())
    }

    async fn lazy_prepare_backend(&self, infra: &Infra, machine_id: MachineId) -> Result<()> {
        use crate::ssh::probe_uname_machine;

        let machine = infra
            .machine_by_id(machine_id)
            .ok_or_else(|| TransportError::Other(format!("unknown machine {machine_id}")))?;
        let choice = infra.transport_for_machine(machine);

        if choice == TransportChoice::SshAgentPush {
            if let MachineKind::Remote { ssh, os } = &machine.kind {
                let uname = os.as_ref().and_then(|h| h.arch.clone());
                if uname.is_none() {
                    let askpass = self.askpass_for(machine_id, ssh).await?;
                    let probed = probe_uname_machine(ssh, askpass.as_deref()).await?;
                    let triple = infrazeug_build::uname_machine_to_triple(&probed);
                    self.machine_triples.lock().await.insert(machine_id, triple);
                }
            }
        }

        self.backend_for(infra, machine_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_core::infra::local_machine;
    use std::time::{Duration, Instant};
    use uuid::Uuid;

    fn mid(seed: u128) -> MachineId {
        MachineId(Uuid::from_u128(seed))
    }

    #[tokio::test]
    async fn execute_streaming_does_not_hold_backend_map_lock() {
        let m1 = mid(1);
        let m2 = mid(2);
        let infra = Infra::new()
            .add_machine(local_machine(m1, "host-a"))
            .unwrap()
            .add_machine(local_machine(m2, "host-b"))
            .unwrap();
        let factory = TransportFactory::new(PathBuf::from("/tmp"), PathBuf::from("/tmp"), false);
        factory.prepare(&infra).await.unwrap();

        let op = ShellOp::run(vec!["sh".into(), "-c".into(), "sleep 0.3".into()]);
        let started = Instant::now();
        let (a, b) = tokio::join!(
            factory.execute_streaming(m1, &op, None),
            factory.execute_streaming(m2, &op, None),
        );

        a.unwrap();
        b.unwrap();
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_millis(550),
            "backend executions were serialized behind the map lock: {elapsed:?}"
        );
    }
}
