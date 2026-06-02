//! Ensure a Keycloak realm user exists and matches a managed set of fields.

use crate::client::KeycloakClientSource;
use async_trait::async_trait;
use infrazeug_ext_keycloak_admin::users::UserRepresentation;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_USER: &str = "keycloak.ensure_user";

/// Tier-1 method: ensure a Keycloak user.
pub type EnsureUser = EnsureResource<UserResource>;

/// Construct the registrable [`EnsureUser`] method for a client source.
pub fn ensure_user(source: KeycloakClientSource) -> EnsureUser {
    EnsureResource::new(UserResource::new(source))
}

/// Desired user — only the managed profile fields. Credentials are intentionally not
/// managed here (set them via a dedicated reset-password node).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureUserInput {
    pub realm: String,
    /// Username — the natural key.
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    /// Defaults to `true` on create when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
}

/// Observed user — the managed fields plus the server id, captured for downstream nodes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureUserOutput {
    /// Server-assigned uuid, used as the `{id}` path segment.
    pub id: String,
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    pub enabled: bool,
    pub email_verified: bool,
}

/// A Keycloak realm user as an acquirable resource.
#[derive(Clone)]
pub struct UserResource {
    source: KeycloakClientSource,
}

impl UserResource {
    pub fn new(source: KeycloakClientSource) -> Self {
        Self { source }
    }
}

fn state_from_rep(rep: UserRepresentation) -> EnsureUserOutput {
    EnsureUserOutput {
        id: rep.id.unwrap_or_default(),
        username: rep.username.unwrap_or_default(),
        email: rep.email,
        first_name: rep.first_name,
        last_name: rep.last_name,
        enabled: rep.enabled.unwrap_or(false),
        email_verified: rep.email_verified.unwrap_or(false),
    }
}

/// Build/patch a [`UserRepresentation`] from the spec. `base` is `None` on create and
/// the live representation on reconcile (so unmanaged fields survive the PUT).
fn rep_from_spec(spec: &EnsureUserInput, base: Option<UserRepresentation>) -> UserRepresentation {
    let mut rep = base.unwrap_or_default();
    rep.username = Some(spec.username.clone());
    if spec.email.is_some() {
        rep.email = spec.email.clone();
    }
    if spec.first_name.is_some() {
        rep.first_name = spec.first_name.clone();
    }
    if spec.last_name.is_some() {
        rep.last_name = spec.last_name.clone();
    }
    match spec.enabled {
        Some(enabled) => rep.enabled = Some(enabled),
        None if rep.enabled.is_none() => rep.enabled = Some(true),
        None => {}
    }
    if let Some(value) = spec.email_verified {
        rep.email_verified = Some(value);
    }
    rep
}

#[async_trait]
impl Resource for UserResource {
    type Spec = EnsureUserInput;
    type State = EnsureUserOutput;

    fn kind(&self) -> &'static str {
        ENSURE_USER
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        // `exact=true` keeps the search from matching usernames by prefix; still confirm
        // equality before adopting the result as the resource's identity.
        let found = client
            .users_search(
                &spec.realm,
                &[("username", spec.username.as_str()), ("exact", "true")],
            )
            .await
            .map_err(ResourceError::provider)?;
        Ok(found
            .into_iter()
            .find(|u| u.username.as_deref() == Some(spec.username.as_str()))
            .map(state_from_rep))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let rep = rep_from_spec(spec, None);
        let id = client
            .create_user(&spec.realm, &rep)
            .await
            .map_err(ResourceError::provider)?;
        let created = client
            .user(&spec.realm, &id)
            .await
            .map_err(ResourceError::provider)?;
        Ok(state_from_rep(created))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if let Some(email) = &spec.email {
            if current.email.as_deref() != Some(email.as_str()) {
                diffs.push(format!("email {:?} → {:?}", current.email, email));
            }
        }
        if let Some(first_name) = &spec.first_name {
            if current.first_name.as_deref() != Some(first_name.as_str()) {
                diffs.push(format!(
                    "first_name {:?} → {:?}",
                    current.first_name, first_name
                ));
            }
        }
        if let Some(last_name) = &spec.last_name {
            if current.last_name.as_deref() != Some(last_name.as_str()) {
                diffs.push(format!(
                    "last_name {:?} → {:?}",
                    current.last_name, last_name
                ));
            }
        }
        if let Some(enabled) = spec.enabled {
            if enabled != current.enabled {
                diffs.push(format!("enabled {} → {}", current.enabled, enabled));
            }
        }
        if let Some(email_verified) = spec.email_verified {
            if email_verified != current.email_verified {
                diffs.push(format!(
                    "email_verified {} → {}",
                    current.email_verified, email_verified
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
        let existing = client
            .user(&spec.realm, &current.id)
            .await
            .map_err(ResourceError::provider)?;
        let rep = rep_from_spec(spec, Some(existing));
        client
            .update_user(&spec.realm, &current.id, &rep)
            .await
            .map_err(ResourceError::provider)?;
        let updated = client
            .user(&spec.realm, &current.id)
            .await
            .map_err(ResourceError::provider)?;
        Ok(state_from_rep(updated))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current() -> EnsureUserOutput {
        EnsureUserOutput {
            id: "uuid".into(),
            username: "alice".into(),
            email: Some("a@x".into()),
            first_name: Some("Alice".into()),
            last_name: None,
            enabled: true,
            email_verified: false,
        }
    }

    fn resource() -> UserResource {
        UserResource::new(KeycloakClientSource::vault("kc.vault"))
    }

    #[test]
    fn unset_fields_do_not_drift() {
        let spec = EnsureUserInput {
            realm: "t".into(),
            username: "alice".into(),
            ..Default::default()
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_email_drifts() {
        let spec = EnsureUserInput {
            realm: "t".into(),
            username: "alice".into(),
            email: Some("b@x".into()),
            ..Default::default()
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }

    #[test]
    fn create_defaults_enabled_true() {
        let spec = EnsureUserInput {
            realm: "t".into(),
            username: "alice".into(),
            ..Default::default()
        };
        let rep = rep_from_spec(&spec, None);
        assert_eq!(rep.enabled, Some(true));
        assert_eq!(rep.username.as_deref(), Some("alice"));
    }
}
