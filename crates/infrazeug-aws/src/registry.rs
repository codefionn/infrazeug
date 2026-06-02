//! Method registry for custom agent binaries linking AWS nodes.

use crate::client::AwsClientSource;
use crate::methods::{ensure_bucket, ensure_iam_access_key, ensure_instance, ensure_volume};
use infrazeug_native::MethodRegistry;

/// Register all AWS tier-1 methods for a shared [`AwsClientSource`].
pub fn method_registry(source: AwsClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    reg.register(ensure_instance(source.clone()));
    reg.register(ensure_volume(source.clone()));
    reg.register(ensure_bucket(source.clone()));
    reg.register(ensure_iam_access_key(source));
    reg
}
