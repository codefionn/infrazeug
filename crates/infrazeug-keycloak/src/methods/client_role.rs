//! Ensure a Keycloak *client* role exists on an OIDC client.
//!
//! Client roles are the `<client>:<role>` form referenced by oauth2-proxy's `allowed_role`.
//! The role's `{id}` path segment is the client's server-assigned uuid, so the resource first
//! resolves the client by its natural `clientId` key, then manages the role under it.

use crate::client::KeycloakClientSource;
use async_trait::async_trait;
use infrazeug_ext_keycloak_admin::roles::RoleRepresentation;
use infrazeug_ext_keycloak_admin::KeycloakClient;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_CLIENT_ROLE: &str = "keycloak.ensure_client_role";

/// Tier-1 method: ensure a Keycloak client role.
pub type EnsureClientRole = EnsureResource<ClientRoleResource>;

/// Construct the registrable [`EnsureClientRole`] method for a client source.
pub fn ensure_client_role(source: KeycloakClientSource) -> EnsureClientRole {
    EnsureResource::new(ClientRoleResource::new(source))
}

/// Desired client role — only the managed fields.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureClientRoleInput {
    pub realm: String,
    /// OIDC `clientId` (natural key) of the client the role lives on.
    pub client_id: String,
    /// Role name — the natural key within the client.
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Observed client role — managed fields plus the resolved client uuid.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureClientRoleOutput {
    /// Server-assigned uuid of the owning client (`{id}` path segment).
    pub client_uuid: String,
    pub client_id: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A Keycloak client role as an acquirable resource.
#[derive(Clone)]
pub struct ClientRoleResource {
    source: KeycloakClientSource,
}

impl ClientRoleResource {
    pub fn new(source: KeycloakClientSource) -> Self {
        Self { source }
    }
}

/// Resolve a client's server-assigned uuid from its `clientId`. `None` when the client does
/// not exist yet (so `observe` reports the role absent rather than erroring).
async fn client_uuid(
    client: &KeycloakClient,
    realm: &str,
    client_id: &str,
) -> ResourceResult<Option<String>> {
    let found = client
        .clients_by_client_id(realm, client_id)
        .await
        .map_err(ResourceError::provider)?;
    Ok(found
        .into_iter()
        .find(|c| c.client_id.as_deref() == Some(client_id))
        .and_then(|c| c.id))
}

#[async_trait]
impl Resource for ClientRoleResource {
    type Spec = EnsureClientRoleInput;
    type State = EnsureClientRoleOutput;

    fn kind(&self) -> &'static str {
        ENSURE_CLIENT_ROLE
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let Some(uuid) = client_uuid(&client, &spec.realm, &spec.client_id).await? else {
            return Ok(None);
        };
        // The client's role list returns full representations; match on the role name
        // rather than the single-role route, which 404s when absent.
        let roles = client
            .client_roles(&spec.realm, &uuid)
            .await
            .map_err(ResourceError::provider)?;
        Ok(roles
            .into_iter()
            .find(|r| r.name.as_deref() == Some(spec.role.as_str()))
            .map(|rep| EnsureClientRoleOutput {
                client_uuid: uuid.clone(),
                client_id: spec.client_id.clone(),
                role: rep.name.unwrap_or_default(),
                description: rep.description,
            }))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let uuid = client_uuid(&client, &spec.realm, &spec.client_id)
            .await?
            .ok_or_else(|| {
                ResourceError::provider(format!(
                    "keycloak client {:?} not found in realm {:?}; ensure the client first",
                    spec.client_id, spec.realm
                ))
            })?;
        let rep = RoleRepresentation {
            name: Some(spec.role.clone()),
            description: spec.description.clone(),
            ..Default::default()
        };
        client
            .create_client_role(&spec.realm, &uuid, &rep)
            .await
            .map_err(ResourceError::provider)?;
        let created = client
            .client_role(&spec.realm, &uuid, &spec.role)
            .await
            .map_err(ResourceError::provider)?;
        Ok(EnsureClientRoleOutput {
            client_uuid: uuid,
            client_id: spec.client_id.clone(),
            role: created.name.unwrap_or_default(),
            description: created.description,
        })
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if let Some(description) = &spec.description {
            if current.description.as_deref() != Some(description.as_str()) {
                diffs.push(format!(
                    "description {:?} → {:?}",
                    current.description, description
                ));
            }
        }
        if diffs.is_empty() {
            Drift::InSync
        } else {
            Drift::Drifted(diffs.join(", "))
        }
    }

    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        // Patch onto the live representation so unmanaged fields survive the PUT.
        let mut rep = client
            .client_role(&spec.realm, &current.client_uuid, &spec.role)
            .await
            .map_err(ResourceError::provider)?;
        if spec.description.is_some() {
            rep.description = spec.description.clone();
        }
        client
            .update_client_role(&spec.realm, &current.client_uuid, &spec.role, &rep)
            .await
            .map_err(ResourceError::provider)?;
        let updated = client
            .client_role(&spec.realm, &current.client_uuid, &spec.role)
            .await
            .map_err(ResourceError::provider)?;
        Ok(EnsureClientRoleOutput {
            client_uuid: current.client_uuid,
            client_id: spec.client_id.clone(),
            role: updated.name.unwrap_or_default(),
            description: updated.description,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> ClientRoleResource {
        ClientRoleResource::new(KeycloakClientSource::vault("kc.vault"))
    }

    fn current() -> EnsureClientRoleOutput {
        EnsureClientRoleOutput {
            client_uuid: "uuid".into(),
            client_id: "hermes-webui".into(),
            role: "hermes-admin".into(),
            description: Some("admins".into()),
        }
    }

    #[test]
    fn same_description_is_in_sync() {
        let spec = EnsureClientRoleInput {
            realm: "default".into(),
            client_id: "hermes-webui".into(),
            role: "hermes-admin".into(),
            description: Some("admins".into()),
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn unset_description_is_in_sync() {
        let spec = EnsureClientRoleInput {
            realm: "default".into(),
            client_id: "hermes-webui".into(),
            role: "hermes-admin".into(),
            description: None,
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_description_drifts() {
        let spec = EnsureClientRoleInput {
            realm: "default".into(),
            client_id: "hermes-webui".into(),
            role: "hermes-admin".into(),
            description: Some("new".into()),
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }
}
