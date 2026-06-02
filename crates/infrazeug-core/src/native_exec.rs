use crate::error::{CoreError, Result};
use crate::id::{MachineId, NodeId};
use crate::infra::Infra;
use crate::machine::MachineKind;
use crate::transport::TransportChoice;
use async_trait::async_trait;
use infrazeug_native::{
    MethodRegistry, NativeResult, NodeCtx, PlanCtx, PlanMethodOutcome, SecretSource,
};
use std::sync::Arc;

/// Executes tier-1 native methods (Local in-process or remote agent RPC).
#[async_trait]
pub trait NativeExecutor: Send + Sync {
    /// Execute a native method.
    ///
    /// `secrets` carries the controller's unlocked vault for `Local` nodes (see
    /// [`VaultSession::secret_source`](crate::VaultSession::secret_source)); it is
    /// `None` for remote agents and when no vault is configured.
    async fn execute_native(
        &self,
        machine_id: MachineId,
        node_id: NodeId,
        method_id: &str,
        input: &serde_cbor::Value,
        secrets: Option<Arc<dyn SecretSource>>,
    ) -> Result<NativeResult>;

    /// Read-only plan-time preview for a native node (powers `--dry-run` / `plan`).
    ///
    /// Defaults to [`PlanMethodOutcome::Unknown`] so executors that cannot reach a
    /// method registry (remote agent transports, [`EmptyNativeExecutor`]) need no
    /// change. Local executors override it to call [`MethodRegistry::plan`].
    async fn plan_native(
        &self,
        _machine_id: MachineId,
        _node_id: NodeId,
        _method_id: &str,
        _input: &serde_cbor::Value,
        _secrets: Option<Arc<dyn SecretSource>>,
    ) -> Result<PlanMethodOutcome> {
        Ok(PlanMethodOutcome::Unknown)
    }
}

/// In-process execution via a [`MethodRegistry`] (controller Local targets).
pub struct LocalNativeExecutor {
    registry: Arc<MethodRegistry>,
}

impl LocalNativeExecutor {
    pub fn new(registry: Arc<MethodRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl NativeExecutor for LocalNativeExecutor {
    async fn execute_native(
        &self,
        machine_id: MachineId,
        node_id: NodeId,
        method_id: &str,
        input: &serde_cbor::Value,
        secrets: Option<Arc<dyn SecretSource>>,
    ) -> Result<NativeResult> {
        let ctx = NodeCtx::new(machine_id.0, node_id.0).with_secrets(secrets);
        self.registry
            .execute(&ctx, method_id, input.clone())
            .await
            .map_err(|e| CoreError::other(e.to_string()))
    }

    async fn plan_native(
        &self,
        machine_id: MachineId,
        node_id: NodeId,
        method_id: &str,
        input: &serde_cbor::Value,
        secrets: Option<Arc<dyn SecretSource>>,
    ) -> Result<PlanMethodOutcome> {
        let ctx = PlanCtx::new(machine_id.0, node_id.0).with_secrets(secrets);
        self.registry
            .plan(&ctx, method_id, input)
            .await
            .map_err(|e| CoreError::other(e.to_string()))
    }
}

/// Routes native execution by machine transport.
pub struct RoutingNativeExecutor {
    infra: Arc<Infra>,
    local: Arc<MethodRegistry>,
    remote: Arc<dyn NativeExecutor>,
}

impl RoutingNativeExecutor {
    pub fn new(
        infra: Arc<Infra>,
        local: Arc<MethodRegistry>,
        remote: Arc<dyn NativeExecutor>,
    ) -> Arc<Self> {
        Arc::new(Self {
            infra,
            local,
            remote,
        })
    }
}

#[async_trait]
impl NativeExecutor for RoutingNativeExecutor {
    async fn execute_native(
        &self,
        machine_id: MachineId,
        node_id: NodeId,
        method_id: &str,
        input: &serde_cbor::Value,
        secrets: Option<Arc<dyn SecretSource>>,
    ) -> Result<NativeResult> {
        let machine = self
            .infra
            .machine_by_id(machine_id)
            .ok_or_else(|| CoreError::other(format!("unknown machine {machine_id}")))?;
        match self.infra.transport_for_machine(machine) {
            TransportChoice::Local => {
                let ctx = NodeCtx::new(machine_id.0, node_id.0).with_secrets(secrets);
                self.local
                    .execute(&ctx, method_id, input.clone())
                    .await
                    .map_err(|e| CoreError::other(e.to_string()))
            }
            TransportChoice::SshAgentPush => {
                // The controller vault stays on the controller; remote agents
                // resolve their own secrets, so no source crosses the RPC boundary.
                self.remote
                    .execute_native(machine_id, node_id, method_id, input, None)
                    .await
            }
            TransportChoice::SshAgentless => Err(CoreError::other(format!(
                "native method `{method_id}` cannot run on agentless machine `{}`",
                machine.name
            ))),
            TransportChoice::PullDaemon => Err(CoreError::other(
                "native methods are not supported in pull-daemon transport",
            )),
        }
    }

    async fn plan_native(
        &self,
        machine_id: MachineId,
        node_id: NodeId,
        method_id: &str,
        input: &serde_cbor::Value,
        secrets: Option<Arc<dyn SecretSource>>,
    ) -> Result<PlanMethodOutcome> {
        let machine = self
            .infra
            .machine_by_id(machine_id)
            .ok_or_else(|| CoreError::other(format!("unknown machine {machine_id}")))?;
        // Only Local (controller) targets are previewable in-process; remote agent
        // transports have no plan-RPC yet, so they stay `Unknown`.
        match self.infra.transport_for_machine(machine) {
            TransportChoice::Local => {
                let ctx = PlanCtx::new(machine_id.0, node_id.0).with_secrets(secrets);
                self.local
                    .plan(&ctx, method_id, input)
                    .await
                    .map_err(|e| CoreError::other(e.to_string()))
            }
            _ => Ok(PlanMethodOutcome::Unknown),
        }
    }
}

/// Plan-time helper: native nodes on containers are unsupported in v1.
pub fn native_supported_on_kind(kind: &MachineKind) -> bool {
    !matches!(kind, MachineKind::Container(_))
}

/// Fallback when no playbook registry is wired (tests / dry paths).
pub struct EmptyNativeExecutor;

#[async_trait]
impl NativeExecutor for EmptyNativeExecutor {
    async fn execute_native(
        &self,
        _machine_id: MachineId,
        _node_id: NodeId,
        method_id: &str,
        _input: &serde_cbor::Value,
        _secrets: Option<Arc<dyn SecretSource>>,
    ) -> Result<NativeResult> {
        Err(CoreError::other(format!(
            "native method `{method_id}`: no native executor configured"
        )))
    }
}

pub fn empty_native_executor() -> Arc<dyn NativeExecutor> {
    Arc::new(EmptyNativeExecutor)
}
