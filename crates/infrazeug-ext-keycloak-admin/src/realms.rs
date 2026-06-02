//! Realm management (`/admin/realms`).
//!
//! Typed bindings for the Keycloak Admin REST API realm endpoints. The
//! [`RealmRepresentation`] model includes the most commonly used fields; less
//! common ones are accessible via the generic `attributes` map.

use crate::client::KeycloakClient;
use crate::error::Result;
use crate::types::{
    EventRepresentation, GlobalRequestResult, RealmEventsConfigRepresentation, RolesRepresentation,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Full Keycloak realm representation.
///
/// Only the most commonly used fields are typed out. All remaining fields are
/// captured in `attributes` and other generic maps, or can be accessed via
/// raw JSON with [`KeycloakClient::get_raw`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RealmRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_before: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoke_refresh_token: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token_max_reuse: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token_lifespan: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sso_session_idle_timeout: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sso_session_max_lifespan: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offline_session_idle_timeout: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offline_session_max_lifespan: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_code_lifespan: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_code_lifespan_user_action: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_token_generated_by_admin_lifespan: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_token_generated_by_user_lifespan: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_required: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_allowed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_email_as_username: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remember_me: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_email: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_with_email_allowed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_emails_allowed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_password_allowed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_username_allowed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brute_force_protected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permanent_lockout: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_failure_wait_seconds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_increment_seconds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_factor: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_role: Option<crate::roles::RoleRepresentation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_groups: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_credentials: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otp_policy_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otp_policy_algorithm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otp_policy_initial_counter: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otp_policy_digits: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otp_policy_look_ahead_window: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otp_policy_period: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_security_headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smtp_server: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login_theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internationalization_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supported_locales: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roles: Option<RolesRepresentation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<crate::groups::GroupRepresentation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_managed_access_allowed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events_expiration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events_listeners: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_event_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_events_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_events_details_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_scopes: Option<Vec<crate::client_scopes::ClientScopeRepresentation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_default_client_scopes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_optional_client_scopes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_grant_flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_credentials_flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_authentication_flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keycloak_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<crate::users::UserRepresentation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clients: Option<Vec<crate::clients::ClientRepresentation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_providers: Option<Vec<crate::identity_providers::IdentityProviderRepresentation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<HashMap<String, Vec<crate::components::ComponentRepresentation>>>,
}

impl KeycloakClient {
    // ── Realm CRUD ───────────────────────────────────────────────────

    /// `GET /admin/realms` — list all realms.
    pub async fn realms(&self) -> Result<Vec<RealmRepresentation>> {
        self.get("").await
    }

    /// `GET /admin/realms/{realm}` — get a single realm.
    pub async fn realm(&self, realm: &str) -> Result<RealmRepresentation> {
        self.get(&format!("/{}", self.encode_path(realm))).await
    }

    /// `POST /admin/realms` — create a new realm.
    pub async fn create_realm(&self, rep: &RealmRepresentation) -> Result<()> {
        self.post("", rep).await
    }

    /// `PUT /admin/realms/{realm}` — update a realm.
    pub async fn update_realm(&self, realm: &str, rep: &RealmRepresentation) -> Result<()> {
        self.put(&format!("/{}", self.encode_path(realm)), rep)
            .await
    }

    /// `DELETE /admin/realms/{realm}` — delete a realm.
    pub async fn delete_realm(&self, realm: &str) -> Result<()> {
        self.delete(&format!("/{}", self.encode_path(realm))).await
    }

    // ── Realm admin operations ───────────────────────────────────────

    /// `POST /admin/realms/{realm}/partial-export` — partial export of realm.
    pub async fn realm_partial_export(
        &self,
        realm: &str,
        export_clients: bool,
        export_groups_and_roles: bool,
    ) -> Result<RealmRepresentation> {
        let body = serde_json::json!({
            "exportClients": export_clients,
            "exportGroupsAndRoles": export_groups_and_roles
        });
        let (_, text) = self
            .request_raw(
                reqwest::Method::POST,
                &format!("/{}/partial-export", self.encode_path(realm)),
                Some(&body.to_string()),
            )
            .await?;
        Ok(serde_json::from_str(&text)?)
    }

    /// `POST /admin/realms/{realm}/partialImport` — partial import into realm.
    pub async fn realm_partial_import(&self, realm: &str, rep: &RealmRepresentation) -> Result<()> {
        self.post(&format!("/{}/partialImport", self.encode_path(realm)), rep)
            .await
    }

    /// `GET /admin/realms/{realm}/events` — list events.
    pub async fn realm_events(&self, realm: &str) -> Result<Vec<EventRepresentation>> {
        self.get(&format!("/{}/events", self.encode_path(realm)))
            .await
    }

    /// `GET /admin/realms/{realm}/events?...` — list events with filters.
    pub async fn realm_events_with_query(
        &self,
        realm: &str,
        query: &[(&str, &str)],
    ) -> Result<Vec<EventRepresentation>> {
        self.get_with_query(&format!("/{}/events", self.encode_path(realm)), query)
            .await
    }

    /// `DELETE /admin/realms/{realm}/events` — clear all events.
    pub async fn realm_clear_events(&self, realm: &str) -> Result<()> {
        self.delete(&format!("/{}/events", self.encode_path(realm)))
            .await
    }

    /// `GET /admin/realms/{realm}/events/config` — get events configuration.
    pub async fn realm_events_config(
        &self,
        realm: &str,
    ) -> Result<RealmEventsConfigRepresentation> {
        self.get(&format!("/{}/events/config", self.encode_path(realm)))
            .await
    }

    /// `PUT /admin/realms/{realm}/events/config` — update events configuration.
    pub async fn realm_update_events_config(
        &self,
        realm: &str,
        config: &RealmEventsConfigRepresentation,
    ) -> Result<()> {
        self.put(
            &format!("/{}/events/config", self.encode_path(realm)),
            config,
        )
        .await
    }

    /// `POST /admin/realms/{realm}/logout-all` — logout all sessions.
    pub async fn realm_logout_all(&self, realm: &str) -> Result<()> {
        self.post(
            &format!("/{}/logout-all", self.encode_path(realm)),
            &serde_json::json!({}),
        )
        .await
    }

    /// `GET /admin/realms/{realm}/session-stats` — get session statistics.
    pub async fn realm_session_stats(
        &self,
        realm: &str,
    ) -> Result<HashMap<String, serde_json::Value>> {
        self.get(&format!("/{}/session-stats", self.encode_path(realm)))
            .await
    }

    /// `POST /admin/realms/{realm}/push-revocation` — push revocation policy.
    pub async fn realm_push_revocation(&self, realm: &str) -> Result<GlobalRequestResult> {
        self.post(
            &format!("/{}/push-revocation", self.encode_path(realm)),
            &serde_json::json!({}),
        )
        .await?;
        Ok(GlobalRequestResult {
            success_requests: None,
            failed_requests: None,
        })
    }

    /// `DELETE /admin/realms/{realm}/admin-events` — clear admin events.
    pub async fn realm_clear_admin_events(&self, realm: &str) -> Result<()> {
        self.delete(&format!("/{}/admin-events", self.encode_path(realm)))
            .await
    }

    /// `GET /admin/realms/{realm}/default-default-client-scopes` — list default default client scopes.
    pub async fn realm_default_default_client_scopes(
        &self,
        realm: &str,
    ) -> Result<Vec<crate::client_scopes::ClientScopeRepresentation>> {
        self.get(&format!(
            "/{}/default-default-client-scopes",
            self.encode_path(realm)
        ))
        .await
    }

    /// `PUT /admin/realms/{realm}/default-default-client-scopes/{clientScopeId}` — add a default default client scope.
    pub async fn realm_add_default_default_client_scope(
        &self,
        realm: &str,
        scope_id: &str,
    ) -> Result<()> {
        self.put(
            &format!(
                "/{}/default-default-client-scopes/{}",
                self.encode_path(realm),
                self.encode_path(scope_id)
            ),
            &serde_json::json!({}),
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/default-default-client-scopes/{clientScopeId}` — remove a default default client scope.
    pub async fn realm_remove_default_default_client_scope(
        &self,
        realm: &str,
        scope_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "/{}/default-default-client-scopes/{}",
            self.encode_path(realm),
            self.encode_path(scope_id)
        ))
        .await
    }

    /// `GET /admin/realms/{realm}/default-optional-client-scopes` — list default optional client scopes.
    pub async fn realm_default_optional_client_scopes(
        &self,
        realm: &str,
    ) -> Result<Vec<crate::client_scopes::ClientScopeRepresentation>> {
        self.get(&format!(
            "/{}/default-optional-client-scopes",
            self.encode_path(realm)
        ))
        .await
    }

    /// `GET /admin/realms/{realm}/group-by-path/{path}` — get group by path.
    pub async fn realm_group_by_path(
        &self,
        realm: &str,
        path: &str,
    ) -> Result<crate::groups::GroupRepresentation> {
        self.get(&format!(
            "/{}/group-by-path/{}",
            self.encode_path(realm),
            self.encode_path(path)
        ))
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_minimal_realm() {
        let json = r#"{"id":"master","realm":"master","enabled":true}"#;
        let r: RealmRepresentation = serde_json::from_str(json).unwrap();
        assert_eq!(r.id.as_deref(), Some("master"));
        assert_eq!(r.realm.as_deref(), Some("master"));
        assert_eq!(r.enabled, Some(true));
    }

    #[test]
    fn deserialize_realm_with_attributes() {
        let json = r#"{
            "realm":"test",
            "enabled":false,
            "attributes":{"custom":"value"},
            "browserFlow":"browser",
            "smtpServer":{"host":"smtp.local","port":"25"}
        }"#;
        let r: RealmRepresentation = serde_json::from_str(json).unwrap();
        assert_eq!(
            r.attributes
                .as_ref()
                .unwrap()
                .get("custom")
                .map(String::as_str),
            Some("value")
        );
        assert_eq!(r.browser_flow.as_deref(), Some("browser"));
        assert_eq!(
            r.smtp_server
                .as_ref()
                .unwrap()
                .get("host")
                .map(String::as_str),
            Some("smtp.local")
        );
    }

    #[test]
    fn serialize_realm_skips_none() {
        let r = RealmRepresentation {
            realm: Some("test".into()),
            enabled: Some(true),
            ..Default::default()
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"realm\":\"test\""));
        assert!(json.contains("\"enabled\":true"));
        assert!(!json.contains("displayName"));
    }
}
