//! Method registry for custom agent binaries linking OpenStack nodes.

use crate::client::OpenstackClientSource;
use crate::methods::{ensure_bucket, ensure_s3_credentials};
use infrazeug_native::MethodRegistry;

/// Register all OpenStack tier-1 methods for a shared [`OpenstackClientSource`].
pub fn method_registry(source: OpenstackClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    reg.register(ensure_s3_credentials(source.clone()));
    reg.register(ensure_bucket(source));
    reg
}
