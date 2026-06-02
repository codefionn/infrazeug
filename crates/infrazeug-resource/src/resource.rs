//! The provider-agnostic [`Resource`] trait and its support types.

use async_trait::async_trait;
use infrazeug_native::{NativeError, NodeCtx, PlanCtx, SecretSource};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Context handed to every [`Resource`] call.
///
/// Constructed from either [`PlanCtx`] (preview/`plan`) or [`NodeCtx`] (apply) so
/// the same `observe` implementation powers both phases. On `Local` (controller)
/// nodes it also carries an unlocked-vault accessor so resources can read provider
/// credentials from the vault instead of the environment (see [`read_secret`]).
///
/// [`read_secret`]: ResourceCtx::read_secret
#[derive(Clone)]
pub struct ResourceCtx {
    pub machine_id: Uuid,
    pub node_id: Uuid,
    secrets: Option<Arc<dyn SecretSource>>,
}

impl ResourceCtx {
    /// Read a vault field (`file`, dot-path `field`) as raw bytes.
    ///
    /// Returns [`ResourceError::SecretsUnavailable`] when no vault session is present
    /// (e.g. a read-only preview): the plan path treats that as "unknown" rather
    /// than a failure.
    pub async fn read_secret(&self, file: &str, field: &str) -> ResourceResult<Vec<u8>> {
        let source = self
            .secrets
            .as_ref()
            .ok_or(ResourceError::SecretsUnavailable)?;
        source
            .read_field(file, field)
            .await
            .map_err(ResourceError::provider)
    }

    /// Read a vault field as a UTF-8 string.
    pub async fn read_secret_string(&self, file: &str, field: &str) -> ResourceResult<String> {
        let bytes = self.read_secret(file, field).await?;
        String::from_utf8(bytes).map_err(|e| {
            ResourceError::provider(format!("vault field {file}:{field} not utf-8: {e}"))
        })
    }

    /// Read a generated mutable-vault field (`files/mutable/{file}`) as raw bytes.
    pub async fn read_mutable_secret(&self, file: &str, field: &str) -> ResourceResult<Vec<u8>> {
        let source = self
            .secrets
            .as_ref()
            .ok_or(ResourceError::InputsUnavailable)?;
        source
            .read_mutable_field(file, field)
            .await
            .map_err(ResourceError::provider)
    }

    /// Read a generated mutable-vault field as a UTF-8 string.
    pub async fn read_mutable_secret_string(
        &self,
        file: &str,
        field: &str,
    ) -> ResourceResult<String> {
        let bytes = self.read_mutable_secret(file, field).await?;
        String::from_utf8(bytes).map_err(|e| {
            ResourceError::provider(format!("mutable vault field {file}:{field} not utf-8: {e}"))
        })
    }

    /// Read an upstream node capture. `machine = None` means this resource's machine.
    pub async fn read_node_capture(
        &self,
        node: Uuid,
        machine: Option<Uuid>,
    ) -> ResourceResult<Vec<u8>> {
        let source = self
            .secrets
            .as_ref()
            .ok_or(ResourceError::InputsUnavailable)?;
        source
            .read_node_capture(node, machine.unwrap_or(self.machine_id))
            .await
            .map_err(ResourceError::provider)
    }

    /// Whether an unlocked controller vault is available on this context.
    pub fn has_secrets(&self) -> bool {
        self.secrets
            .as_ref()
            .map(|source| source.has_vault())
            .unwrap_or(false)
    }

    /// Whether controller-side inputs are available on this context.
    pub fn has_inputs(&self) -> bool {
        self.secrets.is_some()
    }

    /// Whether generated mutable-vault fields are available on this context.
    pub fn has_mutable_secrets(&self) -> bool {
        self.secrets
            .as_ref()
            .map(|source| source.has_mutable_vault())
            .unwrap_or(false)
    }

    /// Whether upstream node captures are available on this context.
    pub fn has_node_captures(&self) -> bool {
        self.secrets
            .as_ref()
            .map(|source| source.has_node_captures())
            .unwrap_or(false)
    }
}

impl std::fmt::Debug for ResourceCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceCtx")
            .field("machine_id", &self.machine_id)
            .field("node_id", &self.node_id)
            .field("inputs", &self.secrets.as_ref().map(|_| "<set>"))
            .finish()
    }
}

impl From<&PlanCtx> for ResourceCtx {
    fn from(ctx: &PlanCtx) -> Self {
        Self {
            machine_id: ctx.machine_id,
            node_id: ctx.node_id,
            secrets: ctx.secrets.clone(),
        }
    }
}

impl From<&NodeCtx> for ResourceCtx {
    fn from(ctx: &NodeCtx) -> Self {
        Self {
            machine_id: ctx.machine_id,
            node_id: ctx.node_id,
            secrets: ctx.secrets.clone(),
        }
    }
}

/// Whether an observed resource matches its desired spec.
///
/// Returned by [`Resource::diff`]; the [`Drift::Drifted`] reason is surfaced
/// in plan previews and the apply message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Drift {
    /// Live state already matches the spec — nothing to do.
    InSync,
    /// Live state differs; the string explains what (e.g. `"region GRA → SBG"`).
    Drifted(String),
}

/// Errors a provider resource may surface.
#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    /// The resource does not implement this part of the lifecycle (e.g. `delete`).
    #[error("resource does not support `{0}`")]
    Unsupported(&'static str),
    /// A credential read needs the controller vault, but none is available on this
    /// context (e.g. read-only preview without an unlocked vault). The plan adapter
    /// maps this to [`PlanMethodOutcome::Unknown`](infrazeug_native::PlanMethodOutcome::Unknown).
    #[error("controller vault unavailable for credential read")]
    SecretsUnavailable,
    /// A dynamic resource input cannot be resolved in this phase/context.
    #[error("controller inputs unavailable for resource input read")]
    InputsUnavailable,
    /// Any provider/API failure, already rendered to a string.
    #[error("{0}")]
    Provider(String),
}

impl ResourceError {
    /// Wrap a provider/ext error message.
    pub fn provider(msg: impl std::fmt::Display) -> Self {
        Self::Provider(msg.to_string())
    }
}

impl From<ResourceError> for NativeError {
    fn from(err: ResourceError) -> Self {
        NativeError::other(err.to_string())
    }
}

pub type ResourceResult<T> = std::result::Result<T, ResourceError>;

/// A typed resource field whose value can be inline or resolved at apply time.
///
/// Use this inside a resource `Spec` when a field may come from an existing vault,
/// a generated mutable-vault file, or a previous node's capture.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "source")]
pub enum ResourceInput<T> {
    Inline {
        value: T,
    },
    Vault {
        file: String,
        field: String,
    },
    MutableVault {
        file: String,
        field: String,
    },
    Node {
        node: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        machine: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        json_pointer: Option<String>,
    },
}

impl<T: Default> Default for ResourceInput<T> {
    fn default() -> Self {
        Self::Inline {
            value: T::default(),
        }
    }
}

impl<T> ResourceInput<T> {
    pub fn inline(value: T) -> Self {
        Self::Inline { value }
    }

    pub fn vault(file: impl Into<String>, field: impl Into<String>) -> Self {
        Self::Vault {
            file: file.into(),
            field: field.into(),
        }
    }

    pub fn mutable_vault(file: impl Into<String>, field: impl Into<String>) -> Self {
        Self::MutableVault {
            file: file.into(),
            field: field.into(),
        }
    }

    pub fn node(node: Uuid) -> Self {
        Self::Node {
            node,
            machine: None,
            json_pointer: None,
        }
    }

    pub fn node_on_machine(node: Uuid, machine: Uuid) -> Self {
        Self::Node {
            node,
            machine: Some(machine),
            json_pointer: None,
        }
    }

    pub fn json_pointer(mut self, pointer: impl Into<String>) -> Self {
        if let Self::Node { json_pointer, .. } = &mut self {
            *json_pointer = Some(pointer.into());
        }
        self
    }
}

impl<T> ResourceInput<T>
where
    T: Clone + DeserializeOwned,
{
    pub async fn resolve(&self, ctx: &ResourceCtx) -> ResourceResult<T> {
        match self {
            Self::Inline { value } => Ok(value.clone()),
            Self::Vault { file, field } => {
                decode_resource_input(&ctx.read_secret(file, field).await?, "vault field")
            }
            Self::MutableVault { file, field } => decode_resource_input(
                &ctx.read_mutable_secret(file, field).await?,
                "mutable vault field",
            ),
            Self::Node {
                node,
                machine,
                json_pointer,
            } => {
                let bytes = ctx.read_node_capture(*node, *machine).await?;
                if let Some(pointer) = json_pointer {
                    return decode_json_pointer(&bytes, pointer);
                }
                decode_resource_input(&bytes, "node capture")
            }
        }
    }
}

fn decode_resource_input<T: DeserializeOwned>(bytes: &[u8], label: &str) -> ResourceResult<T> {
    match serde_json::from_slice(bytes) {
        Ok(value) => Ok(value),
        Err(json_err) => {
            if let Ok(text) = std::str::from_utf8(bytes) {
                return serde_json::from_value(serde_json::Value::String(text.to_string()))
                    .map_err(|string_err| {
                        ResourceError::provider(format!(
                            "{label} did not decode as JSON ({json_err}) or string ({string_err})"
                        ))
                    });
            }
            let byte_array = serde_json::Value::Array(
                bytes
                    .iter()
                    .map(|b| serde_json::Value::Number((*b).into()))
                    .collect(),
            );
            serde_json::from_value(byte_array).map_err(|bytes_err| {
                ResourceError::provider(format!(
                    "{label} did not decode as JSON ({json_err}) or bytes ({bytes_err})"
                ))
            })
        }
    }
}

fn decode_json_pointer<T: DeserializeOwned>(bytes: &[u8], pointer: &str) -> ResourceResult<T> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| ResourceError::provider(format!("node capture is not JSON: {e}")))?;
    let resolved = value.pointer(pointer).ok_or_else(|| {
        ResourceError::provider(format!("node capture JSON pointer {pointer:?} not found"))
    })?;
    serde_json::from_value(resolved.clone())
        .map_err(|e| ResourceError::provider(format!("node capture JSON pointer {pointer:?}: {e}")))
}

/// A single acquirable resource (bucket, server, volume, user, …).
///
/// Implement this for each provider surface and wrap it in
/// [`EnsureResource`](crate::EnsureResource) to obtain a tier-1
/// [`NodeMethod`](infrazeug_native::NodeMethod) — the resource then participates
/// in the node graph (deps, run-policy, capture→vault, retry) like any other node,
/// with idempotency, plan/diff, and reconcile handled once by the adapter.
///
/// The minimum a provider must supply is [`observe`](Self::observe) and
/// [`create`](Self::create). Override [`diff`](Self::diff) +
/// [`reconcile`](Self::reconcile) to manage drift, and [`delete`](Self::delete) for
/// teardown (both default to "no drift" / "unsupported").
#[async_trait]
pub trait Resource: Send + Sync + 'static {
    /// Desired state — the node input. Must round-trip through CBOR/JSON.
    type Spec: DeserializeOwned + Serialize + Default + Clone + Send + Sync;
    /// Observed/created live resource — the node output (captured for vault writes).
    type State: Serialize + DeserializeOwned + Send + Sync;

    /// Stable identifier, reused verbatim as the
    /// [`NodeMethod::name`](infrazeug_native::NodeMethod::name) / method id
    /// (e.g. `"ovh.ensure_storage_container"`).
    fn kind(&self) -> &'static str;

    /// Read the current live resource matching `spec`, or `None` if it is absent.
    ///
    /// This is the single source of idempotency: it runs at plan time (preview)
    /// and at the start of apply. Must be read-only.
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>>;

    /// Create the resource described by `spec` (only called when `observe` is `None`).
    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State>;

    /// Decide whether an existing `current` already satisfies `spec`.
    ///
    /// Defaults to [`Drift::InSync`] (create-if-absent only). Override to detect
    /// drift; if this can return [`Drift::Drifted`], also override
    /// [`reconcile`](Self::reconcile).
    fn diff(&self, _spec: &Self::Spec, _current: &Self::State) -> Drift {
        Drift::InSync
    }

    /// Reconcile a drifted `current` toward `spec`, returning the updated state.
    ///
    /// Defaults to a no-op that returns `current` unchanged.
    async fn reconcile(
        &self,
        _ctx: &ResourceCtx,
        _spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        Ok(current)
    }

    /// Destroy the resource. Defaults to [`ResourceError::Unsupported`].
    ///
    /// Not invoked by the current ensure adapter; provided so destroy/teardown can
    /// be wired later without changing the trait.
    async fn delete(&self, _ctx: &ResourceCtx, _state: &Self::State) -> ResourceResult<()> {
        Err(ResourceError::Unsupported("delete"))
    }
}
