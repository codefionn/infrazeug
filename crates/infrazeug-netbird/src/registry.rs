//! Registration for custom agents linking NetBird methods.
use crate::*;
use infrazeug_native::MethodRegistry;

/// Register all NetBird resource adapters for one client source.
pub fn method_registry(source: NetBirdClientSource) -> MethodRegistry {
    let mut registry = MethodRegistry::new();
    registry.register(ensure_group(source.clone()));
    registry.register(ensure_identity_provider(source.clone()));
    registry.register(ensure_policy(source.clone()));
    registry.register(ensure_route(source.clone()));
    registry.register(ensure_network(source.clone()));
    registry.register(ensure_network_resource(source.clone()));
    registry.register(ensure_network_router(source.clone()));
    registry.register(ensure_nameserver_group(source.clone()));
    registry.register(ensure_reverse_proxy_domain(source.clone()));
    registry.register(ensure_reverse_proxy_service(source.clone()));
    registry.register(ensure_setup_key(source.clone()));
    registry.register(ensure_reverse_proxy_token(source.clone()));
    registry.register(ensure_dns_settings(source));
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registers_all_provider_methods() {
        let registry = method_registry(NetBirdClientSource::vault("netbird.vault"));
        for method in [
            ENSURE_GROUP,
            ENSURE_IDENTITY_PROVIDER,
            ENSURE_POLICY,
            ENSURE_ROUTE,
            ENSURE_NETWORK,
            ENSURE_NETWORK_RESOURCE,
            ENSURE_NETWORK_ROUTER,
            ENSURE_NAMESERVER_GROUP,
            ENSURE_REVERSE_PROXY_DOMAIN,
            ENSURE_REVERSE_PROXY_SERVICE,
            ENSURE_SETUP_KEY,
            ENSURE_REVERSE_PROXY_TOKEN,
            ENSURE_DNS_SETTINGS,
        ] {
            assert!(registry.get(method).is_some(), "missing {method}");
        }
    }
}
