//! Method registry for custom agent binaries linking IONOS nodes.

use crate::client::IonosClientSource;
use crate::methods::{ensure_datacenter, ensure_server, ensure_volume};
use infrazeug_native::MethodRegistry;

/// Register all IONOS tier-1 methods for a shared [`IonosClientSource`].
///
/// Accepts a ready [`IonosClient`](infrazeug_ext_ionos_cloud_api::IonosClient) (e.g.
/// `IonosClientSource::ready(client_from_env()?)`) or vault-backed credentials
/// (`IonosClientSource::vault("cloud/ionos.vault")`).
pub fn method_registry(source: IonosClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    reg.register(ensure_datacenter(source.clone()));
    reg.register(ensure_server(source.clone()));
    reg.register(ensure_volume(source));
    reg
}
