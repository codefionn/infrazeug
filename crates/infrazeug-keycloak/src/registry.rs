//! Method registry for custom agent binaries linking Keycloak nodes.

use crate::client::KeycloakClientSource;
use crate::methods::{ensure_client, ensure_client_role, ensure_realm, ensure_user};
use infrazeug_native::MethodRegistry;

/// Register all Keycloak tier-1 methods for a shared [`KeycloakClientSource`].
///
/// Accepts any source: a ready [`KeycloakClient`](infrazeug_ext_keycloak_admin::KeycloakClient)
/// (e.g. `KeycloakClientSource::ready(client_from_env()?)`) or vault-backed credentials
/// (`KeycloakClientSource::vault("auth/keycloak.vault")`).
pub fn method_registry(source: KeycloakClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    reg.register(ensure_realm(source.clone()));
    reg.register(ensure_client(source.clone()));
    reg.register(ensure_client_role(source.clone()));
    reg.register(ensure_user(source));
    reg
}
