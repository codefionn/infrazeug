use crate::error::{NativeError, Result};
use serde::Serialize;

/// Serialize a native method input for storage on a native node body.
///
/// Produces a CBOR map/array value (not an opaque byte blob) so plan fingerprints stay
/// human-inspectable and inputs round-trip through [`decode_input`](decode_input).
pub fn encode_input<T: Serialize>(input: &T) -> Result<serde_cbor::Value> {
    let bytes = serde_cbor::to_vec(input).map_err(|e| NativeError::other(e.to_string()))?;
    serde_cbor::from_slice(&bytes).map_err(|e| NativeError::other(e.to_string()))
}

/// Deserialize input stored on a native node body.
#[allow(dead_code)]
pub fn decode_input<T: serde::de::DeserializeOwned + Default>(
    value: serde_cbor::Value,
) -> Result<T> {
    crate::method::decode_input(value).map_err(NativeError::other)
}
