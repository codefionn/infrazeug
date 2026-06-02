use crate::error::{NativeError, Result};
use crate::result::NativeResult;
use crate::secret::SecretSource;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

/// Plan-time context for [`NodeMethod::plan`] (apply-only v1 uses `Unknown`).
#[derive(Clone)]
pub struct PlanCtx {
    pub machine_id: Uuid,
    pub node_id: Uuid,
    /// Controller vault access for `Local` nodes (preview without an unlocked vault
    /// leaves this `None`).
    pub secrets: Option<Arc<dyn SecretSource>>,
}

/// Apply-time context passed to [`NodeMethod::execute`].
#[derive(Clone)]
pub struct NodeCtx {
    pub machine_id: Uuid,
    pub node_id: Uuid,
    /// Controller vault access for `Local` nodes (remote agents get `None`).
    pub secrets: Option<Arc<dyn SecretSource>>,
}

impl PlanCtx {
    /// Context without vault access.
    pub fn new(machine_id: Uuid, node_id: Uuid) -> Self {
        Self {
            machine_id,
            node_id,
            secrets: None,
        }
    }

    /// Attach a controller secret source (chainable).
    pub fn with_secrets(mut self, secrets: Option<Arc<dyn SecretSource>>) -> Self {
        self.secrets = secrets;
        self
    }
}

impl NodeCtx {
    /// Context without vault access.
    pub fn new(machine_id: Uuid, node_id: Uuid) -> Self {
        Self {
            machine_id,
            node_id,
            secrets: None,
        }
    }

    /// Attach a controller secret source (chainable).
    pub fn with_secrets(mut self, secrets: Option<Arc<dyn SecretSource>>) -> Self {
        self.secrets = secrets;
        self
    }
}

impl std::fmt::Debug for PlanCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlanCtx")
            .field("machine_id", &self.machine_id)
            .field("node_id", &self.node_id)
            .field("secrets", &self.secrets.as_ref().map(|_| "<set>"))
            .finish()
    }
}

impl std::fmt::Debug for NodeCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeCtx")
            .field("machine_id", &self.machine_id)
            .field("node_id", &self.node_id)
            .field("secrets", &self.secrets.as_ref().map(|_| "<set>"))
            .finish()
    }
}

/// Outcome of plan-time inspection (deferred enrichment in a later milestone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanMethodOutcome {
    Unchanged,
    Changed,
    Unknown,
}

/// Typed tier-1 method implemented by playbooks and built-ins.
#[async_trait]
pub trait NodeMethod: Send + Sync {
    type Input: DeserializeOwned + Serialize + Default + Send + Sync;
    type Output: Serialize + Send + Sync;

    fn name(&self) -> &'static str;

    fn idempotent(&self) -> bool {
        false
    }

    async fn plan(&self, _ctx: &PlanCtx, _input: &Self::Input) -> Result<PlanMethodOutcome> {
        Ok(PlanMethodOutcome::Unknown)
    }

    async fn execute(&self, _ctx: &NodeCtx, input: Self::Input) -> Result<NativeResult>;
}

#[async_trait]
pub trait ErasedNodeMethod: Send + Sync {
    fn name(&self) -> &str;

    fn idempotent(&self) -> bool;

    async fn plan_erased(
        &self,
        ctx: &PlanCtx,
        input: &serde_cbor::Value,
    ) -> Result<PlanMethodOutcome>;

    async fn execute_erased(&self, ctx: &NodeCtx, input: serde_cbor::Value)
        -> Result<NativeResult>;
}

pub struct TypedMethod<M: NodeMethod> {
    inner: M,
}

impl<M: NodeMethod> TypedMethod<M> {
    pub fn new(inner: M) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<M: NodeMethod> ErasedNodeMethod for TypedMethod<M> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn idempotent(&self) -> bool {
        self.inner.idempotent()
    }

    async fn plan_erased(
        &self,
        ctx: &PlanCtx,
        input: &serde_cbor::Value,
    ) -> Result<PlanMethodOutcome> {
        let typed: M::Input =
            cbor_to_input(input.clone()).map_err(|e| NativeError::InvalidInput {
                method: self.inner.name().to_string(),
                detail: e.to_string(),
            })?;
        self.inner.plan(ctx, &typed).await
    }

    async fn execute_erased(
        &self,
        ctx: &NodeCtx,
        input: serde_cbor::Value,
    ) -> Result<NativeResult> {
        let typed: M::Input = cbor_to_input(input).map_err(|e| NativeError::InvalidInput {
            method: self.inner.name().to_string(),
            detail: e.to_string(),
        })?;
        let out = self.inner.execute(ctx, typed).await?;
        Ok(out)
    }
}

pub fn erase<M: NodeMethod + 'static>(method: M) -> std::sync::Arc<dyn ErasedNodeMethod> {
    std::sync::Arc::new(TypedMethod::new(method))
}

pub(crate) fn decode_input<T: DeserializeOwned + Default>(
    value: serde_cbor::Value,
) -> std::result::Result<T, String> {
    match value {
        serde_cbor::Value::Bytes(bytes) => {
            serde_cbor::from_slice(&bytes).map_err(|e| e.to_string())
        }
        serde_cbor::Value::Null => Ok(T::default()),
        other => {
            let bytes = serde_cbor::to_vec(&other).map_err(|e| e.to_string())?;
            serde_cbor::from_slice(&bytes).map_err(|e| e.to_string())
        }
    }
}

fn cbor_to_input<T: DeserializeOwned + Default>(
    value: serde_cbor::Value,
) -> std::result::Result<T, String> {
    decode_input(value)
}
