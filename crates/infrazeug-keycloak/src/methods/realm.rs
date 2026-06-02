//! Ensure a Keycloak realm exists and matches a small set of managed attributes.

use crate::client::KeycloakClientSource;
use async_trait::async_trait;
use infrazeug_ext_keycloak_admin::realms::RealmRepresentation;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_REALM: &str = "keycloak.ensure_realm";

/// Tier-1 method: ensure a Keycloak realm.
pub type EnsureRealm = EnsureResource<RealmResource>;

/// Construct the registrable [`EnsureRealm`] method for a client source.
pub fn ensure_realm(source: KeycloakClientSource) -> EnsureRealm {
    EnsureResource::new(RealmResource::new(source))
}

/// Desired realm — only the managed fields. Everything else is left to Keycloak's
/// defaults on create and preserved as-is on reconcile.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureRealmInput {
    /// Realm name — the natural key (`realm` in the Keycloak representation).
    pub realm: String,
    /// Defaults to `true` on create when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_theme: Option<String>,
}

/// Observed realm — the managed fields, captured for downstream nodes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureRealmOutput {
    pub realm: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_theme: Option<String>,
}

/// A Keycloak realm as an acquirable resource.
#[derive(Clone)]
pub struct RealmResource {
    source: KeycloakClientSource,
}

impl RealmResource {
    pub fn new(source: KeycloakClientSource) -> Self {
        Self { source }
    }
}

fn state_from_rep(rep: RealmRepresentation) -> EnsureRealmOutput {
    EnsureRealmOutput {
        realm: rep.realm.unwrap_or_default(),
        enabled: rep.enabled.unwrap_or(false),
        display_name: rep.display_name,
        login_theme: rep.login_theme,
    }
}

#[async_trait]
impl Resource for RealmResource {
    type Spec = EnsureRealmInput;
    type State = EnsureRealmOutput;

    fn kind(&self) -> &'static str {
        ENSURE_REALM
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        // The realm list is small and returns full representations; match on the realm
        // name (its natural key) rather than calling the single-realm route, which
        // would 404 when absent.
        let realms = client.realms().await.map_err(ResourceError::provider)?;
        Ok(realms
            .into_iter()
            .find(|r| r.realm.as_deref() == Some(spec.realm.as_str()))
            .map(state_from_rep))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let rep = RealmRepresentation {
            realm: Some(spec.realm.clone()),
            enabled: Some(spec.enabled.unwrap_or(true)),
            display_name: spec.display_name.clone(),
            login_theme: spec.login_theme.clone(),
            ..Default::default()
        };
        client
            .create_realm(&rep)
            .await
            .map_err(ResourceError::provider)?;
        // Re-read so the captured state reflects the realm as Keycloak stored it.
        let created = client
            .realm(&spec.realm)
            .await
            .map_err(ResourceError::provider)?;
        Ok(state_from_rep(created))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if let Some(enabled) = spec.enabled {
            if enabled != current.enabled {
                diffs.push(format!("enabled {} → {}", current.enabled, enabled));
            }
        }
        if let Some(display_name) = &spec.display_name {
            if current.display_name.as_deref() != Some(display_name.as_str()) {
                diffs.push(format!(
                    "display_name {:?} → {:?}",
                    current.display_name, display_name
                ));
            }
        }
        if let Some(login_theme) = &spec.login_theme {
            if current.login_theme.as_deref() != Some(login_theme.as_str()) {
                diffs.push(format!(
                    "login_theme {:?} → {:?}",
                    current.login_theme, login_theme
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
        _current: Self::State,
    ) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        // GET the full representation so fields outside the managed set survive the PUT.
        let mut rep = client
            .realm(&spec.realm)
            .await
            .map_err(ResourceError::provider)?;
        if let Some(enabled) = spec.enabled {
            rep.enabled = Some(enabled);
        }
        if spec.display_name.is_some() {
            rep.display_name = spec.display_name.clone();
        }
        if spec.login_theme.is_some() {
            rep.login_theme = spec.login_theme.clone();
        }
        client
            .update_realm(&spec.realm, &rep)
            .await
            .map_err(ResourceError::provider)?;
        Ok(state_from_rep(rep))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current() -> EnsureRealmOutput {
        EnsureRealmOutput {
            realm: "t".into(),
            enabled: true,
            display_name: Some("Tenant".into()),
            login_theme: None,
        }
    }

    fn resource() -> RealmResource {
        RealmResource::new(KeycloakClientSource::vault("kc.vault"))
    }

    #[test]
    fn unset_fields_do_not_drift() {
        let spec = EnsureRealmInput {
            realm: "t".into(),
            ..Default::default()
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_field_drifts() {
        let spec = EnsureRealmInput {
            realm: "t".into(),
            display_name: Some("Renamed".into()),
            ..Default::default()
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }

    #[test]
    fn matching_field_is_in_sync() {
        let spec = EnsureRealmInput {
            realm: "t".into(),
            enabled: Some(true),
            display_name: Some("Tenant".into()),
            ..Default::default()
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }
}
