//! Ensure a Keycloak (OIDC) client exists and matches a managed set of fields.

use crate::client::KeycloakClientSource;
use async_trait::async_trait;
use infrazeug_ext_keycloak_admin::clients::ClientRepresentation;
use infrazeug_ext_keycloak_admin::KeycloakClient;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_CLIENT: &str = "keycloak.ensure_client";

/// Tier-1 method: ensure a Keycloak client.
pub type EnsureClient = EnsureResource<ClientResource>;

/// Construct the registrable [`EnsureClient`] method for a client source.
pub fn ensure_client(source: KeycloakClientSource) -> EnsureClient {
    EnsureResource::new(ClientResource::new(source))
}

/// Desired client — only the managed fields.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureClientInput {
    pub realm: String,
    /// OIDC `clientId` — the natural key (not the server-assigned uuid).
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Defaults to `true` on create when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_client: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standard_flow_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_accounts_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_uris: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_origins: Option<Vec<String>>,
}

/// Observed client — managed fields plus the confidential secret, captured for
/// downstream vault writes (mirrors the OVH S3-user credential capture).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureClientOutput {
    /// Server-assigned uuid (`id`), used as the `{id}` path segment.
    pub id: String,
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub enabled: bool,
    pub public_client: bool,
    pub standard_flow_enabled: bool,
    pub service_accounts_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_uris: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_origins: Option<Vec<String>>,
    /// Confidential client secret. Absent for public clients (or when the client has no
    /// secret-based authenticator), so an optional vault pointer skips it cleanly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

/// A Keycloak OIDC client as an acquirable resource.
#[derive(Clone)]
pub struct ClientResource {
    source: KeycloakClientSource,
}

impl ClientResource {
    pub fn new(source: KeycloakClientSource) -> Self {
        Self { source }
    }
}

/// Build/patch a [`ClientRepresentation`] from the spec. `base` is `None` on create
/// (so unset booleans fall to Keycloak defaults, except `enabled` → `true`) and the
/// fetched representation on reconcile (so unmanaged fields are preserved).
fn rep_from_spec(
    spec: &EnsureClientInput,
    base: Option<ClientRepresentation>,
) -> ClientRepresentation {
    let mut rep = base.unwrap_or_default();
    rep.client_id = Some(spec.client_id.clone());
    if let Some(name) = &spec.name {
        rep.name = Some(name.clone());
    }
    match spec.enabled {
        Some(enabled) => rep.enabled = Some(enabled),
        None if rep.enabled.is_none() => rep.enabled = Some(true),
        None => {}
    }
    if let Some(value) = spec.public_client {
        rep.public_client = Some(value);
    }
    if let Some(value) = spec.standard_flow_enabled {
        rep.standard_flow_enabled = Some(value);
    }
    if let Some(value) = spec.service_accounts_enabled {
        rep.service_accounts_enabled = Some(value);
    }
    if spec.redirect_uris.is_some() {
        rep.redirect_uris = spec.redirect_uris.clone();
    }
    if spec.web_origins.is_some() {
        rep.web_origins = spec.web_origins.clone();
    }
    rep
}

/// Map a representation to the captured state, best-effort reading the secret for
/// confidential clients.
async fn state_from_rep(
    client: &KeycloakClient,
    realm: &str,
    rep: ClientRepresentation,
) -> ResourceResult<EnsureClientOutput> {
    let id = rep.id.clone().unwrap_or_default();
    let public_client = rep.public_client.unwrap_or(false);
    let secret = if public_client || id.is_empty() {
        None
    } else {
        // Best effort: confidential clients expose a secret here; clients using a
        // non-secret authenticator (jwt, x509) have none, so ignore the failure.
        client
            .client_secret(realm, &id)
            .await
            .ok()
            .and_then(|cred| cred.value)
    };
    Ok(EnsureClientOutput {
        id,
        client_id: rep.client_id.unwrap_or_default(),
        name: rep.name,
        enabled: rep.enabled.unwrap_or(false),
        public_client,
        standard_flow_enabled: rep.standard_flow_enabled.unwrap_or(false),
        service_accounts_enabled: rep.service_accounts_enabled.unwrap_or(false),
        redirect_uris: rep.redirect_uris,
        web_origins: rep.web_origins,
        secret,
    })
}

/// Order-insensitive comparison: Keycloak may reorder list-valued fields.
fn list_drifts(current: Option<&Vec<String>>, desired: &[String]) -> bool {
    let mut a = current.cloned().unwrap_or_default();
    let mut b = desired.to_vec();
    a.sort();
    b.sort();
    a != b
}

#[async_trait]
impl Resource for ClientResource {
    type Spec = EnsureClientInput;
    type State = EnsureClientOutput;

    fn kind(&self) -> &'static str {
        ENSURE_CLIENT
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        // `clientId` is filtered server-side, but confirm an exact match before
        // adopting the result as the resource's identity.
        let found = client
            .clients_by_client_id(&spec.realm, &spec.client_id)
            .await
            .map_err(ResourceError::provider)?;
        let Some(rep) = found
            .into_iter()
            .find(|c| c.client_id.as_deref() == Some(spec.client_id.as_str()))
        else {
            return Ok(None);
        };
        Ok(Some(state_from_rep(&client, &spec.realm, rep).await?))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let rep = rep_from_spec(spec, None);
        let id = client
            .create_client(&spec.realm, &rep)
            .await
            .map_err(ResourceError::provider)?;
        // Re-read so the captured state reflects server-filled defaults and the secret.
        let created = client
            .client(&spec.realm, &id)
            .await
            .map_err(ResourceError::provider)?;
        state_from_rep(&client, &spec.realm, created).await
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if let Some(name) = &spec.name {
            if current.name.as_deref() != Some(name.as_str()) {
                diffs.push(format!("name {:?} → {:?}", current.name, name));
            }
        }
        if let Some(value) = spec.enabled {
            if value != current.enabled {
                diffs.push(format!("enabled {} → {}", current.enabled, value));
            }
        }
        if let Some(value) = spec.public_client {
            if value != current.public_client {
                diffs.push(format!(
                    "public_client {} → {}",
                    current.public_client, value
                ));
            }
        }
        if let Some(value) = spec.standard_flow_enabled {
            if value != current.standard_flow_enabled {
                diffs.push(format!(
                    "standard_flow_enabled {} → {}",
                    current.standard_flow_enabled, value
                ));
            }
        }
        if let Some(value) = spec.service_accounts_enabled {
            if value != current.service_accounts_enabled {
                diffs.push(format!(
                    "service_accounts_enabled {} → {}",
                    current.service_accounts_enabled, value
                ));
            }
        }
        if let Some(uris) = &spec.redirect_uris {
            if list_drifts(current.redirect_uris.as_ref(), uris) {
                diffs.push("redirect_uris".to_string());
            }
        }
        if let Some(origins) = &spec.web_origins {
            if list_drifts(current.web_origins.as_ref(), origins) {
                diffs.push("web_origins".to_string());
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
        let existing = client
            .client(&spec.realm, &current.id)
            .await
            .map_err(ResourceError::provider)?;
        let rep = rep_from_spec(spec, Some(existing));
        client
            .update_client(&spec.realm, &current.id, &rep)
            .await
            .map_err(ResourceError::provider)?;
        let updated = client
            .client(&spec.realm, &current.id)
            .await
            .map_err(ResourceError::provider)?;
        state_from_rep(&client, &spec.realm, updated).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current() -> EnsureClientOutput {
        EnsureClientOutput {
            id: "uuid".into(),
            client_id: "web".into(),
            name: Some("Web".into()),
            enabled: true,
            public_client: false,
            standard_flow_enabled: true,
            service_accounts_enabled: false,
            redirect_uris: Some(vec!["https://a/".into(), "https://b/".into()]),
            web_origins: None,
            secret: Some("s3cr3t".into()),
        }
    }

    fn resource() -> ClientResource {
        ClientResource::new(KeycloakClientSource::vault("kc.vault"))
    }

    #[test]
    fn redirect_uri_reorder_is_not_drift() {
        let spec = EnsureClientInput {
            realm: "t".into(),
            client_id: "web".into(),
            redirect_uris: Some(vec!["https://b/".into(), "https://a/".into()]),
            ..Default::default()
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_flag_drifts() {
        let spec = EnsureClientInput {
            realm: "t".into(),
            client_id: "web".into(),
            public_client: Some(true),
            ..Default::default()
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }

    #[test]
    fn create_defaults_enabled_true() {
        let spec = EnsureClientInput {
            realm: "t".into(),
            client_id: "web".into(),
            ..Default::default()
        };
        let rep = rep_from_spec(&spec, None);
        assert_eq!(rep.enabled, Some(true));
        assert_eq!(rep.client_id.as_deref(), Some("web"));
    }

    #[test]
    fn reconcile_preserves_base_enabled_when_unset() {
        let spec = EnsureClientInput {
            realm: "t".into(),
            client_id: "web".into(),
            ..Default::default()
        };
        let base = ClientRepresentation {
            enabled: Some(false),
            ..Default::default()
        };
        let rep = rep_from_spec(&spec, Some(base));
        assert_eq!(rep.enabled, Some(false));
    }
}
