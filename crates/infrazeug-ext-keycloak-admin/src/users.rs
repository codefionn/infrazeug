//! User management (`/admin/realms/{realm}/users`).
//!
//! Full user lifecycle: create, read, update, delete, credentials, role
//! mappings, sessions, groups, federated identities, and impersonation.

use crate::client::KeycloakClient;
use crate::error::Result;
use crate::types::{
    CredentialRepresentation, FederatedIdentityRepresentation, MappingsRepresentation,
    UserConsentRepresentation, UserSessionRepresentation,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totp: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub federation_link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account_client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Vec<CredentialRepresentation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disableable_credential_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_actions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub federated_identities: Option<Vec<FederatedIdentityRepresentation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm_roles: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_roles: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_consents: Option<Vec<UserConsentRepresentation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_before: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<HashMap<String, bool>>,
}

fn realm_path(realm: &str) -> String {
    format!("/{}", urlencoding::encode(realm))
}

fn user_base(realm: &str) -> String {
    format!("{}/users", realm_path(realm))
}

impl KeycloakClient {
    /// `GET /admin/realms/{realm}/users` — list users (optionally filtered).
    pub async fn users(&self, realm: &str) -> Result<Vec<UserRepresentation>> {
        self.get(&user_base(realm)).await
    }

    /// `GET /admin/realms/{realm}/users?...` — search users with query params.
    ///
    /// Common params: `search`, `username`, `email`, `firstName`, `lastName`,
    /// `first`, `max`, `briefRepresentation`.
    pub async fn users_search(
        &self,
        realm: &str,
        query: &[(&str, &str)],
    ) -> Result<Vec<UserRepresentation>> {
        self.get_with_query(&user_base(realm), query).await
    }

    /// `GET /admin/realms/{realm}/users/{id}` — get a single user.
    pub async fn user(&self, realm: &str, user_id: &str) -> Result<UserRepresentation> {
        self.get(&format!(
            "{}/{}",
            user_base(realm),
            self.encode_path(user_id)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/users` — create a user. Returns the new user ID.
    pub async fn create_user(&self, realm: &str, user: &UserRepresentation) -> Result<String> {
        self.post_and_extract_id(&user_base(realm), user).await
    }

    /// `PUT /admin/realms/{realm}/users/{id}` — update a user.
    pub async fn update_user(
        &self,
        realm: &str,
        user_id: &str,
        user: &UserRepresentation,
    ) -> Result<()> {
        self.put(
            &format!("{}/{}", user_base(realm), self.encode_path(user_id)),
            user,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/users/{id}` — delete a user.
    pub async fn delete_user(&self, realm: &str, user_id: &str) -> Result<()> {
        self.delete(&format!(
            "{}/{}",
            user_base(realm),
            self.encode_path(user_id)
        ))
        .await
    }

    /// `GET /admin/realms/{realm}/users/{id}/groups` — get groups the user belongs to.
    pub async fn user_groups(
        &self,
        realm: &str,
        user_id: &str,
    ) -> Result<Vec<crate::groups::GroupRepresentation>> {
        self.get(&format!(
            "{}/{}/groups",
            user_base(realm),
            self.encode_path(user_id)
        ))
        .await
    }

    /// `PUT /admin/realms/{realm}/users/{id}/groups/{groupId}` — add user to group.
    pub async fn user_join_group(&self, realm: &str, user_id: &str, group_id: &str) -> Result<()> {
        self.put(
            &format!(
                "{}/{}/groups/{}",
                user_base(realm),
                self.encode_path(user_id),
                self.encode_path(group_id)
            ),
            &serde_json::json!({}),
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/users/{id}/groups/{groupId}` — remove user from group.
    pub async fn user_leave_group(&self, realm: &str, user_id: &str, group_id: &str) -> Result<()> {
        self.delete(&format!(
            "{}/{}/groups/{}",
            user_base(realm),
            self.encode_path(user_id),
            self.encode_path(group_id)
        ))
        .await
    }

    // ── Credentials ──────────────────────────────────────────────────

    /// `PUT /admin/realms/{realm}/users/{id}/reset-password` — reset user password.
    pub async fn user_reset_password(
        &self,
        realm: &str,
        user_id: &str,
        cred: &CredentialRepresentation,
    ) -> Result<()> {
        self.put(
            &format!(
                "{}/{}/reset-password",
                user_base(realm),
                self.encode_path(user_id)
            ),
            cred,
        )
        .await
    }

    /// `GET /admin/realms/{realm}/users/{id}/credentials` — list user credentials.
    pub async fn user_credentials(
        &self,
        realm: &str,
        user_id: &str,
    ) -> Result<Vec<CredentialRepresentation>> {
        self.get(&format!(
            "{}/{}/credentials",
            user_base(realm),
            self.encode_path(user_id)
        ))
        .await
    }

    /// `DELETE /admin/realms/{realm}/users/{id}/credentials/{credentialId}` — delete a credential.
    pub async fn user_delete_credential(
        &self,
        realm: &str,
        user_id: &str,
        credential_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "{}/{}/credentials/{}",
            user_base(realm),
            self.encode_path(user_id),
            self.encode_path(credential_id)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/users/{id}/credentials/{credentialId}/moveToFirst` — move credential to first.
    pub async fn user_credential_move_to_first(
        &self,
        realm: &str,
        user_id: &str,
        credential_id: &str,
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/credentials/{}/moveToFirst",
                user_base(realm),
                self.encode_path(user_id),
                self.encode_path(credential_id)
            ),
            &serde_json::json!({}),
        )
        .await
    }

    // ── Role mappings ────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/users/{id}/role-mappings` — get all role mappings.
    pub async fn user_role_mappings(
        &self,
        realm: &str,
        user_id: &str,
    ) -> Result<MappingsRepresentation> {
        self.get(&format!(
            "{}/{}/role-mappings",
            user_base(realm),
            self.encode_path(user_id)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/users/{id}/role-mappings/realm` — add realm role mappings.
    pub async fn user_add_realm_roles(
        &self,
        realm: &str,
        user_id: &str,
        roles: &[crate::roles::RoleRepresentation],
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/role-mappings/realm",
                user_base(realm),
                self.encode_path(user_id)
            ),
            roles,
        )
        .await
    }

    /// `GET /admin/realms/{realm}/users/{id}/role-mappings/realm` — get realm role mappings.
    pub async fn user_realm_roles(
        &self,
        realm: &str,
        user_id: &str,
    ) -> Result<Vec<crate::roles::RoleRepresentation>> {
        self.get(&format!(
            "{}/{}/role-mappings/realm",
            user_base(realm),
            self.encode_path(user_id)
        ))
        .await
    }

    /// `DELETE /admin/realms/{realm}/users/{id}/role-mappings/realm` — remove realm role mappings.
    pub async fn user_remove_realm_roles(
        &self,
        realm: &str,
        user_id: &str,
        roles: &[crate::roles::RoleRepresentation],
    ) -> Result<()> {
        let body = serde_json::to_string(roles)?;
        self.request_raw(
            reqwest::Method::DELETE,
            &format!(
                "{}/{}/role-mappings/realm",
                user_base(realm),
                self.encode_path(user_id)
            ),
            Some(&body),
        )
        .await?;
        Ok(())
    }

    /// `POST /admin/realms/{realm}/users/{id}/role-mappings/clients/{client}` — add client role mappings.
    pub async fn user_add_client_roles(
        &self,
        realm: &str,
        user_id: &str,
        client_id: &str,
        roles: &[crate::roles::RoleRepresentation],
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/role-mappings/clients/{}",
                user_base(realm),
                self.encode_path(user_id),
                self.encode_path(client_id)
            ),
            roles,
        )
        .await
    }

    /// `GET /admin/realms/{realm}/users/{id}/role-mappings/clients/{client}` — get client role mappings.
    pub async fn user_client_roles(
        &self,
        realm: &str,
        user_id: &str,
        client_id: &str,
    ) -> Result<Vec<crate::roles::RoleRepresentation>> {
        self.get(&format!(
            "{}/{}/role-mappings/clients/{}",
            user_base(realm),
            self.encode_path(user_id),
            self.encode_path(client_id)
        ))
        .await
    }

    // ── Sessions ─────────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/users/{id}/sessions` — list user sessions.
    pub async fn user_sessions(
        &self,
        realm: &str,
        user_id: &str,
    ) -> Result<Vec<UserSessionRepresentation>> {
        self.get(&format!(
            "{}/{}/sessions",
            user_base(realm),
            self.encode_path(user_id)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/users/{id}/logout` — logout user from all sessions.
    pub async fn user_logout(&self, realm: &str, user_id: &str) -> Result<()> {
        self.post(
            &format!("{}/{}/logout", user_base(realm), self.encode_path(user_id)),
            &serde_json::json!({}),
        )
        .await
    }

    // ── Consents ─────────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/users/{id}/consents` — list user consents.
    pub async fn user_consents(
        &self,
        realm: &str,
        user_id: &str,
    ) -> Result<Vec<UserConsentRepresentation>> {
        self.get(&format!(
            "{}/{}/consents",
            user_base(realm),
            self.encode_path(user_id)
        ))
        .await
    }

    // ── Federated identity ───────────────────────────────────────────

    /// `GET /admin/realms/{realm}/users/{id}/federated-identity` — list federated identities.
    pub async fn user_federated_identity(
        &self,
        realm: &str,
        user_id: &str,
    ) -> Result<Vec<FederatedIdentityRepresentation>> {
        self.get(&format!(
            "{}/{}/federated-identity",
            user_base(realm),
            self.encode_path(user_id)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/users/{id}/federated-identity/{provider}` — add federated identity.
    pub async fn user_add_federated_identity(
        &self,
        realm: &str,
        user_id: &str,
        provider: &str,
        rep: &FederatedIdentityRepresentation,
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/federated-identity/{}",
                user_base(realm),
                self.encode_path(user_id),
                self.encode_path(provider)
            ),
            rep,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/users/{id}/federated-identity/{provider}` — remove federated identity.
    pub async fn user_remove_federated_identity(
        &self,
        realm: &str,
        user_id: &str,
        provider: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "{}/{}/federated-identity/{}",
            user_base(realm),
            self.encode_path(user_id),
            self.encode_path(provider)
        ))
        .await
    }

    // ── Required actions ─────────────────────────────────────────────

    /// `PUT /admin/realms/{realm}/users/{id}/execute-actions-email` — send execute actions email.
    pub async fn user_execute_actions_email(
        &self,
        realm: &str,
        user_id: &str,
        actions: &[String],
    ) -> Result<()> {
        self.put(
            &format!(
                "{}/{}/execute-actions-email",
                user_base(realm),
                self.encode_path(user_id)
            ),
            actions,
        )
        .await
    }

    /// `PUT /admin/realms/{realm}/users/{id}/send-verify-email` — send verification email.
    pub async fn user_send_verify_email(&self, realm: &str, user_id: &str) -> Result<()> {
        self.put(
            &format!(
                "{}/{}/send-verify-email",
                user_base(realm),
                self.encode_path(user_id)
            ),
            &serde_json::json!({}),
        )
        .await
    }

    // ── Impersonation ────────────────────────────────────────────────

    /// `POST /admin/realms/{realm}/users/{id}/impersonation` — impersonate user.
    pub async fn user_impersonate(&self, realm: &str, user_id: &str) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/impersonation",
                user_base(realm),
                self.encode_path(user_id)
            ),
            &serde_json::json!({}),
        )
        .await
    }

    /// `GET /admin/realms/{realm}/users/{id}/unreadmgmtperm` — get management permissions.
    pub async fn user_management_permissions(
        &self,
        realm: &str,
        user_id: &str,
    ) -> Result<crate::types::ManagementPermissionReference> {
        self.get(&format!(
            "{}/{}/management/permissions",
            user_base(realm),
            self.encode_path(user_id)
        ))
        .await
    }

    /// `PUT /admin/realms/{realm}/users/{id}/management/permissions` — update management permissions.
    pub async fn user_update_management_permissions(
        &self,
        realm: &str,
        user_id: &str,
        perm: &crate::types::ManagementPermissionReference,
    ) -> Result<crate::types::ManagementPermissionReference> {
        self.put_json(
            &format!(
                "{}/{}/management/permissions",
                user_base(realm),
                self.encode_path(user_id)
            ),
            perm,
        )
        .await
    }

    /// `GET /admin/realms/{realm}/users/count` — count users.
    pub async fn users_count(&self, realm: &str) -> Result<i64> {
        let (_, text) = self
            .request_raw(
                reqwest::Method::GET,
                &format!("/{}/users/count", urlencoding::encode(realm)),
                None,
            )
            .await?;
        text.trim()
            .parse::<i64>()
            .map_err(|e| crate::error::KeycloakError::Auth(format!("invalid count: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_minimal_user() {
        let json = r#"{"id":"abc","username":"alice","enabled":true,"emailVerified":false}"#;
        let u: UserRepresentation = serde_json::from_str(json).unwrap();
        assert_eq!(u.id.as_deref(), Some("abc"));
        assert_eq!(u.username.as_deref(), Some("alice"));
        assert_eq!(u.enabled, Some(true));
        assert_eq!(u.email_verified, Some(false));
    }

    #[test]
    fn deserialize_user_with_attributes() {
        let json = r#"{
            "id":"abc",
            "username":"bob",
            "attributes":{"phone":["+123"],"department":["eng"]},
            "realmRoles":["admin","user"],
            "requiredActions":["VERIFY_EMAIL"]
        }"#;
        let u: UserRepresentation = serde_json::from_str(json).unwrap();
        assert_eq!(u.attributes.unwrap().get("phone").unwrap().len(), 1);
        assert_eq!(u.realm_roles.unwrap().len(), 2);
        assert_eq!(u.required_actions.unwrap()[0], "VERIFY_EMAIL");
    }

    #[test]
    fn serialize_user_skips_none() {
        let u = UserRepresentation {
            username: Some("alice".into()),
            enabled: Some(true),
            ..Default::default()
        };
        let json = serde_json::to_string(&u).unwrap();
        assert!(json.contains("\"username\":\"alice\""));
        assert!(!json.contains("firstName"));
    }
}
