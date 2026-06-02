//! Method registry for custom agent binaries linking Backblaze nodes.

use crate::client::BackblazeClientSource;
use crate::methods::{ensure_application_key, ensure_bucket};
use infrazeug_native::MethodRegistry;

/// Register all Backblaze tier-1 methods for a shared [`BackblazeClientSource`].
pub fn method_registry(source: BackblazeClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    let source_keys = source.clone();
    reg.register(ensure_bucket(source_keys));
    reg.register(ensure_application_key(source));
    reg
}
