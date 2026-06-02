//! Method registry for custom agent binaries linking OVH nodes.

use crate::client::OvhClientSource;
use crate::methods::{
    ensure_instance, ensure_s3_user, ensure_s3_user_policy, ensure_storage_container,
};
use infrazeug_native::MethodRegistry;

/// Register all OVH tier-1 methods for a shared [`OvhClientSource`].
///
/// Accepts any source: a ready [`OvhClient`](infrazeug_ext_ovh_api::OvhClient) (e.g.
/// `OvhClientSource::ready(client_from_env()?)`) or vault-backed credentials
/// (`OvhClientSource::vault("cloud/ovh.vault")`).
pub fn method_registry(source: OvhClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    reg.register(ensure_storage_container(source.clone()));
    reg.register(ensure_s3_user(source.clone()));
    reg.register(ensure_s3_user_policy(source.clone()));
    reg.register(ensure_instance(source));
    reg
}
