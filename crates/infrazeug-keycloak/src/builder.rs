//! Fluent infra builder extension for Keycloak native nodes.

use crate::client::KeycloakClientSource;
use crate::methods::{
    ensure_client, ensure_client_role, ensure_realm, ensure_user, EnsureClient, EnsureClientInput,
    EnsureClientRole, EnsureClientRoleInput, EnsureRealm, EnsureRealmInput, EnsureUser,
    EnsureUserInput,
};
use infrazeug_api::builder::InfraBuilder;
use infrazeug_api::PlaybookBundle;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_ext_keycloak_admin::KeycloakClient;

/// Extension trait: attach Keycloak methods to an [`InfraBuilder`].
pub trait KeycloakInfraExt {
    /// Register Keycloak methods against a ready client (e.g. from `client_from_env`).
    fn keycloak(self, client: KeycloakClient, machine_id: MachineId) -> KeycloakInfraBuilder;

    /// Register Keycloak methods that read their `client_credentials` from the controller
    /// vault at apply time (no `KEYCLOAK_*` environment variables needed).
    ///
    /// `file` is the vault file (under `files/`) holding `base_url`, `client_secret`, and
    /// optionally `realm`/`client_id`; its DataKey must be among the run's unlocked keys.
    fn keycloak_vault(self, file: impl Into<String>, machine_id: MachineId)
        -> KeycloakInfraBuilder;

    /// Register Keycloak methods that authenticate with a `password` (direct-access) grant,
    /// reading both the username and the password from *existing* vault fields at apply time.
    ///
    /// Lets a dedicated API admin (`vault_keycloak_api_admin_user` /
    /// `vault_keycloak_api_admin_password`) be reused without sealing a dedicated service-account
    /// client. `base_url`/`token_realm`/`client_id` are non-secret config (e.g. `https://id…`,
    /// `master`, `admin-cli`); `username_field` and `password_field` in `file` are the secrets.
    /// The file's DataKey must be among the run's unlocked keys.
    #[allow(clippy::too_many_arguments)]
    fn keycloak_vault_password(
        self,
        base_url: impl Into<String>,
        token_realm: impl Into<String>,
        client_id: impl Into<String>,
        file: impl Into<String>,
        username_field: impl Into<String>,
        password_field: impl Into<String>,
        machine_id: MachineId,
    ) -> KeycloakInfraBuilder;
}

impl KeycloakInfraExt for InfraBuilder {
    fn keycloak(self, client: KeycloakClient, machine_id: MachineId) -> KeycloakInfraBuilder {
        KeycloakInfraBuilder::new(self, KeycloakClientSource::ready(client), machine_id)
    }

    fn keycloak_vault(
        self,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> KeycloakInfraBuilder {
        KeycloakInfraBuilder::new(self, KeycloakClientSource::vault(file), machine_id)
    }

    fn keycloak_vault_password(
        self,
        base_url: impl Into<String>,
        token_realm: impl Into<String>,
        client_id: impl Into<String>,
        file: impl Into<String>,
        username_field: impl Into<String>,
        password_field: impl Into<String>,
        machine_id: MachineId,
    ) -> KeycloakInfraBuilder {
        KeycloakInfraBuilder::new(
            self,
            KeycloakClientSource::vault_password(
                base_url,
                token_realm,
                client_id,
                file,
                username_field,
                password_field,
            ),
            machine_id,
        )
    }
}

/// Staged builder with Keycloak methods pre-registered.
pub struct KeycloakInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}

impl KeycloakInfraBuilder {
    pub fn new(builder: InfraBuilder, source: KeycloakClientSource, machine_id: MachineId) -> Self {
        let builder = builder
            .method(ensure_realm(source.clone()))
            .method(ensure_client(source.clone()))
            .method(ensure_client_role(source.clone()))
            .method(ensure_user(source));
        Self {
            builder,
            machine_id,
        }
    }

    /// Ensure a realm exists (create or reconcile to the managed fields).
    pub fn ensure_realm(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureRealmInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureRealm>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Ensure an OIDC client exists (create or reconcile to the managed fields).
    pub fn ensure_client(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureClientInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureClient>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Ensure a *client* role exists on an OIDC client (create or reconcile).
    ///
    /// A client role's path is keyed by the client's uuid, so this must run after the client
    /// exists — pass the `ensure_client` node id(s) as `deps`.
    pub fn ensure_client_role(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureClientRoleInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureClientRole>(node_id, name, self.machine_id, input)?
            .deps(deps)
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Ensure a realm user exists (create or reconcile to the managed fields).
    pub fn ensure_user(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureUserInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureUser>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }

    /// Return the underlying [`InfraBuilder`] for further chaining.
    pub fn into_builder(self) -> InfraBuilder {
        self.builder
    }

    /// Finish as a [`PlaybookBundle`].
    pub fn finish(self) -> PlaybookBundle {
        self.builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_api::builder;
    use infrazeug_ext_keycloak_admin::{GrantType, KeycloakConfig};
    use uuid::Uuid;

    fn dummy_client() -> KeycloakClient {
        KeycloakClient::new(KeycloakConfig::new(
            "http://kc.local",
            "master",
            GrantType::ClientCredentials {
                client_id: "admin-cli".into(),
                client_secret: "secret".into(),
            },
        ))
    }

    #[test]
    fn realm_client_user_plans() {
        let local = MachineId(Uuid::new_v4());
        let realm = NodeId(Uuid::new_v4());
        let client = NodeId(Uuid::new_v4());
        let user = NodeId(Uuid::new_v4());

        let bundle = InfraBuilder::new()
            .machine(builder::controller(local))
            .unwrap()
            .keycloak(dummy_client(), local)
            .ensure_realm(
                realm,
                "tenant-realm",
                EnsureRealmInput {
                    realm: "tenant".into(),
                    enabled: Some(true),
                    display_name: Some("Tenant".into()),
                    ..Default::default()
                },
            )
            .unwrap()
            .ensure_client(
                client,
                "tenant-web",
                EnsureClientInput {
                    realm: "tenant".into(),
                    client_id: "web".into(),
                    standard_flow_enabled: Some(true),
                    redirect_uris: Some(vec!["https://app.local/*".into()]),
                    ..Default::default()
                },
            )
            .unwrap()
            .ensure_user(
                user,
                "tenant-admin",
                EnsureUserInput {
                    realm: "tenant".into(),
                    username: "admin".into(),
                    email: Some("admin@tenant.local".into()),
                    ..Default::default()
                },
            )
            .unwrap()
            .finish();

        // `build()` injects a per-machine connectivity head plus the global
        // begin/finish bookends; count the real (user-authored) nodes.
        let real_nodes = bundle
            .infra
            .nodes
            .iter()
            .filter(|n| !(n.body.is_group_bookend() || n.body.is_connect()))
            .count();
        assert_eq!(real_nodes, 3);
        bundle.plan().expect("lint + plan");
    }
}
