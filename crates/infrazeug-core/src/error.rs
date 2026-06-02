use crate::lint::LintReport;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Clone, Debug, Error)]
pub enum CoreError {
    #[error("duplicate {kind} name: {name}")]
    DuplicateName { kind: &'static str, name: String },

    #[error("unknown machine: {0}")]
    UnknownMachine(String),

    #[error("unknown group: {0}")]
    UnknownGroup(String),

    #[error("node `{node}` depends on unknown node `{dep}`")]
    UnknownDependency { node: String, dep: String },

    #[error("cycle in graph: {0}")]
    Cycle(String),

    #[error("{0}")]
    Lint(LintReport),

    #[error("plan drift: file digest {file} != recomputed {recomputed}")]
    PlanDrift { file: String, recomputed: String },

    #[error("interaction denied: {0}")]
    InteractionDenied(String),

    #[error("interaction cancelled")]
    InteractionCancelled,

    #[error("node `{node}` cannot run native method on agentless machine `{machine}`")]
    NativeOnAgentless { node: String, machine: String },

    #[error("node `{node}` cannot run native method on container machine `{machine}`")]
    NativeOnContainer { node: String, machine: String },

    #[error(
        "node `{node}` references unknown native method `{method}` for local machine `{machine}`"
    )]
    NativeMethodNotRegistered {
        node: String,
        method: String,
        machine: String,
    },

    #[error("lazy node `{node}` has no non-lazy dependent and can never be demanded")]
    LazyNodeUndemandable { node: String },

    #[error(
        "pull slice for `{node}` requires cross-machine wait on `{dependency}` (use push mode)"
    )]
    PullSliceNeedsWait { node: String, dependency: String },

    #[error("node `{consumer}` references unknown capture node `{node}`")]
    CaptureUnknownNode { consumer: String, node: String },

    #[error("node `{consumer}` capture from `{upstream}` requires `{upstream}` in deps")]
    CaptureNotInDeps { consumer: String, upstream: String },

    #[error("node `{consumer}` capture from `{upstream}` on machine `{machine}` is not a target of `{upstream}`")]
    CaptureInvalidMachine {
        consumer: String,
        upstream: String,
        machine: String,
    },

    #[error("capture for node `{node}` on machine `{machine}` missing at apply time")]
    CaptureMissing { node: String, machine: String },

    #[error("capture for node `{node}` on machine `{machine}` is {bytes} bytes (limit {limit})")]
    CaptureTooLarge {
        node: String,
        machine: String,
        bytes: usize,
        limit: usize,
    },

    #[error("plaintext secret in {location}: {what}")]
    PlaintextSecret { location: String, what: String },

    #[error("{0}")]
    Other(String),
}

impl CoreError {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Stable, kebab-case identifier for this error kind. Used by lint
    /// diagnostics so tooling can match on a code rather than message text.
    pub fn code(&self) -> &'static str {
        match self {
            Self::DuplicateName { .. } => "duplicate-name",
            Self::UnknownMachine(_) => "unknown-machine",
            Self::UnknownGroup(_) => "unknown-group",
            Self::UnknownDependency { .. } => "unknown-dependency",
            Self::Cycle(_) => "cycle",
            Self::Lint(_) => "lint",
            Self::PlanDrift { .. } => "plan-drift",
            Self::InteractionDenied(_) => "interaction-denied",
            Self::InteractionCancelled => "interaction-cancelled",
            Self::NativeOnAgentless { .. } => "native-on-agentless",
            Self::NativeOnContainer { .. } => "native-on-container",
            Self::NativeMethodNotRegistered { .. } => "native-method-not-registered",
            Self::LazyNodeUndemandable { .. } => "lazy-node-undemandable",
            Self::PullSliceNeedsWait { .. } => "pull-slice-needs-wait",
            Self::CaptureUnknownNode { .. } => "capture-unknown-node",
            Self::CaptureNotInDeps { .. } => "capture-not-in-deps",
            Self::CaptureInvalidMachine { .. } => "capture-invalid-machine",
            Self::CaptureMissing { .. } => "capture-missing",
            Self::CaptureTooLarge { .. } => "capture-too-large",
            Self::PlaintextSecret { .. } => "plaintext-secret",
            Self::Other(_) => "other",
        }
    }
}

impl From<serde_cbor::Error> for CoreError {
    fn from(e: serde_cbor::Error) -> Self {
        Self::Other(e.to_string())
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(e: serde_json::Error) -> Self {
        Self::Other(e.to_string())
    }
}

impl From<serde_yaml::Error> for CoreError {
    fn from(e: serde_yaml::Error) -> Self {
        Self::Other(e.to_string())
    }
}

impl From<toml::ser::Error> for CoreError {
    fn from(e: toml::ser::Error) -> Self {
        Self::Other(e.to_string())
    }
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Other(e.to_string())
    }
}

impl From<infrazeug_secrets::SecretsError> for CoreError {
    fn from(e: infrazeug_secrets::SecretsError) -> Self {
        Self::Other(e.to_string())
    }
}
