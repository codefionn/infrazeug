use serde::{Deserialize, Serialize};

/// JSON capture bytes for downstream [`FileSource::Capture`] / `VaultWrite` nodes.
pub type NativeCapture = Vec<u8>;

/// Whether a native method run changed remote/controller state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NativeStatus {
    Changed,
    Unchanged,
}

/// Result of a native method `execute` call (wire + scheduler).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeResult {
    pub status: NativeStatus,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub output: Option<serde_cbor::Value>,
    /// JSON document for the capture store (same role as shell stdout).
    #[serde(default)]
    pub capture: Option<NativeCapture>,
}

impl NativeResult {
    pub fn changed(message: impl Into<String>) -> Self {
        Self {
            status: NativeStatus::Changed,
            message: Some(message.into()),
            output: None,
            capture: None,
        }
    }

    pub fn unchanged(message: impl Into<String>) -> Self {
        Self {
            status: NativeStatus::Unchanged,
            message: Some(message.into()),
            output: None,
            capture: None,
        }
    }

    pub fn with_output(mut self, output: serde_cbor::Value) -> Self {
        self.output = Some(output);
        self
    }

    /// Attach JSON capture bytes for downstream shell [`VaultWrite`] / `WriteFile` nodes.
    pub fn with_json_capture<T: Serialize>(mut self, value: &T) -> Result<Self, serde_json::Error> {
        self.capture = Some(serde_json::to_vec(value)?);
        Ok(self)
    }
}
