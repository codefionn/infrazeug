//! Method registry for custom agent binaries linking STACKIT nodes.

use crate::client::StackitClientSource;
use crate::methods::{ensure_server, ensure_volume};
use infrazeug_native::MethodRegistry;

/// Register all STACKIT tier-1 methods for a shared [`StackitClientSource`].
pub fn method_registry(source: StackitClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    reg.register(ensure_server(source.clone()));
    reg.register(ensure_volume(source));
    reg
}
