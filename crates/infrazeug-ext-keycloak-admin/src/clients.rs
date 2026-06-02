//! Client management (`/admin/realms/{realm}/clients`).
//!
//! Full client lifecycle: CRUD, secrets, scopes, sessions, protocol mappers,
//! service accounts, and certificates.

use crate::client::KeycloakClient;
use crate::error::Result;
use crate::types::{
    CertificateRepresentation, CredentialRepresentation, ManagementPermissionReference,
    ProtocolMapperRepresentation, UserSessionRepresentation,
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surrogate_auth_required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always_display_in_console: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_authenticator_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uris: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_origins: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_before: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearer_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent_required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub standard_flow_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implicit_flow_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_access_grants_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_accounts_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_client: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontchannel_logout: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_scope_allowed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_mappers: Option<Vec<ProtocolMapperRepresentation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_client_scopes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional_client_scopes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_services_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<HashMap<String, bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

fn client_base(realm: &str) -> String {
    format!("/{}/clients", urlencoding::encode(realm))
}

impl KeycloakClient {
    /// `GET /admin/realms/{realm}/clients` — list all clients.
    pub async fn clients(&self, realm: &str) -> Result<Vec<ClientRepresentation>> {
        self.get(&client_base(realm)).await
    }

    /// `GET /admin/realms/{realm}/clients?clientId=...` — find clients by clientId.
    pub async fn clients_by_client_id(
        &self,
        realm: &str,
        client_id: &str,
    ) -> Result<Vec<ClientRepresentation>> {
        self.get_with_query(&client_base(realm), &[("clientId", client_id)])
            .await
    }

    /// `GET /admin/realms/{realm}/clients/{id}` — get a single client.
    pub async fn client(&self, realm: &str, id: &str) -> Result<ClientRepresentation> {
        self.get(&format!("{}/{}", client_base(realm), self.encode_path(id)))
            .await
    }

    /// `POST /admin/realms/{realm}/clients` — create a client. Returns the new client ID.
    pub async fn create_client(&self, realm: &str, rep: &ClientRepresentation) -> Result<String> {
        self.post_and_extract_id(&client_base(realm), rep).await
    }

    /// `PUT /admin/realms/{realm}/clients/{id}` — update a client.
    pub async fn update_client(
        &self,
        realm: &str,
        id: &str,
        rep: &ClientRepresentation,
    ) -> Result<()> {
        self.put(
            &format!("{}/{}", client_base(realm), self.encode_path(id)),
            rep,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/clients/{id}` — delete a client.
    pub async fn delete_client(&self, realm: &str, id: &str) -> Result<()> {
        self.delete(&format!("{}/{}", client_base(realm), self.encode_path(id)))
            .await
    }

    // ── Client secret ────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/clients/{id}/client-secret` — get client secret.
    pub async fn client_secret(&self, realm: &str, id: &str) -> Result<CredentialRepresentation> {
        self.get(&format!(
            "{}/{}/client-secret",
            client_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/clients/{id}/client-secret` — regenerate client secret.
    pub async fn client_regenerate_secret(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<CredentialRepresentation> {
        self.request_raw(
            Method::POST,
            &format!(
                "{}/{}/client-secret",
                client_base(realm),
                self.encode_path(id)
            ),
            None,
        )
        .await?;
        self.client_secret(realm, id).await
    }

    // ── Sessions ─────────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/clients/{id}/user-sessions` — list client sessions.
    pub async fn client_user_sessions(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<Vec<UserSessionRepresentation>> {
        self.get(&format!(
            "{}/{}/user-sessions",
            client_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `GET /admin/realms/{realm}/clients/{id}/offline-sessions` — list offline sessions.
    pub async fn client_offline_sessions(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<Vec<UserSessionRepresentation>> {
        self.get(&format!(
            "{}/{}/offline-sessions",
            client_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    // ── Scopes ───────────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/clients/{id}/default-client-scopes` — list default client scopes.
    pub async fn client_default_client_scopes(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<Vec<crate::client_scopes::ClientScopeRepresentation>> {
        self.get(&format!(
            "{}/{}/default-client-scopes",
            client_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `PUT /admin/realms/{realm}/clients/{id}/default-client-scopes/{scopeId}` — add default scope.
    pub async fn client_add_default_scope(
        &self,
        realm: &str,
        id: &str,
        scope_id: &str,
    ) -> Result<()> {
        self.put(
            &format!(
                "{}/{}/default-client-scopes/{}",
                client_base(realm),
                self.encode_path(id),
                self.encode_path(scope_id)
            ),
            &serde_json::json!({}),
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/clients/{id}/default-client-scopes/{scopeId}` — remove default scope.
    pub async fn client_remove_default_scope(
        &self,
        realm: &str,
        id: &str,
        scope_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "{}/{}/default-client-scopes/{}",
            client_base(realm),
            self.encode_path(id),
            self.encode_path(scope_id)
        ))
        .await
    }

    /// `GET /admin/realms/{realm}/clients/{id}/optional-client-scopes` — list optional client scopes.
    pub async fn client_optional_client_scopes(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<Vec<crate::client_scopes::ClientScopeRepresentation>> {
        self.get(&format!(
            "{}/{}/optional-client-scopes",
            client_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `PUT /admin/realms/{realm}/clients/{id}/optional-client-scopes/{scopeId}` — add optional scope.
    pub async fn client_add_optional_scope(
        &self,
        realm: &str,
        id: &str,
        scope_id: &str,
    ) -> Result<()> {
        self.put(
            &format!(
                "{}/{}/optional-client-scopes/{}",
                client_base(realm),
                self.encode_path(id),
                self.encode_path(scope_id)
            ),
            &serde_json::json!({}),
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/clients/{id}/optional-client-scopes/{scopeId}` — remove optional scope.
    pub async fn client_remove_optional_scope(
        &self,
        realm: &str,
        id: &str,
        scope_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "{}/{}/optional-client-scopes/{}",
            client_base(realm),
            self.encode_path(id),
            self.encode_path(scope_id)
        ))
        .await
    }

    // ── Protocol mappers ─────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/clients/{id}/protocol-mappers` — list protocol mappers.
    pub async fn client_protocol_mappers(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<Vec<ProtocolMapperRepresentation>> {
        self.get(&format!(
            "{}/{}/protocol-mappers/models",
            client_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/clients/{id}/protocol-mappers/models` — add protocol mapper.
    pub async fn client_add_protocol_mapper(
        &self,
        realm: &str,
        id: &str,
        mapper: &ProtocolMapperRepresentation,
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/protocol-mappers/models",
                client_base(realm),
                self.encode_path(id)
            ),
            mapper,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/clients/{id}/protocol-mappers/models/{mapperId}` — delete mapper.
    pub async fn client_delete_protocol_mapper(
        &self,
        realm: &str,
        id: &str,
        mapper_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "{}/{}/protocol-mappers/models/{}",
            client_base(realm),
            self.encode_path(id),
            self.encode_path(mapper_id)
        ))
        .await
    }

    // ── Service account ──────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/clients/{id}/service-account-user` — get service account user.
    pub async fn client_service_account_user(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<crate::users::UserRepresentation> {
        self.get(&format!(
            "{}/{}/service-account-user",
            client_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    // ── Certificates ─────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/clients/{id}/certificates/{attr}` — get certificate info.
    pub async fn client_certificate(
        &self,
        realm: &str,
        id: &str,
        attr: &str,
    ) -> Result<CertificateRepresentation> {
        self.get(&format!(
            "{}/{}/certificates/{}",
            client_base(realm),
            self.encode_path(id),
            self.encode_path(attr)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/clients/{id}/certificates/{attr}/generate` — generate certificate.
    pub async fn client_generate_certificate(
        &self,
        realm: &str,
        id: &str,
        attr: &str,
    ) -> Result<CertificateRepresentation> {
        let (_, body) = self
            .request_raw(
                Method::POST,
                &format!(
                    "{}/{}/certificates/{}/generate",
                    client_base(realm),
                    self.encode_path(id),
                    self.encode_path(attr)
                ),
                None,
            )
            .await?;
        Ok(serde_json::from_str(&body)?)
    }

    // ── Management permissions ───────────────────────────────────────

    /// `GET /admin/realms/{realm}/clients/{id}/management/permissions` — get management permissions.
    pub async fn client_management_permissions(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<ManagementPermissionReference> {
        self.get(&format!(
            "{}/{}/management/permissions",
            client_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `PUT /admin/realms/{realm}/clients/{id}/management/permissions` — update management permissions.
    pub async fn client_update_management_permissions(
        &self,
        realm: &str,
        id: &str,
        perm: &ManagementPermissionReference,
    ) -> Result<ManagementPermissionReference> {
        self.put_json(
            &format!(
                "{}/{}/management/permissions",
                client_base(realm),
                self.encode_path(id)
            ),
            perm,
        )
        .await
    }

    // ── Revocation ───────────────────────────────────────────────────

    /// `POST /admin/realms/{realm}/clients/{id}/push-revocation` — push revocation.
    pub async fn client_push_revocation(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<crate::types::GlobalRequestResult> {
        let (_, body) = self
            .request_raw(
                Method::POST,
                &format!(
                    "{}/{}/push-revocation",
                    client_base(realm),
                    self.encode_path(id)
                ),
                None,
            )
            .await?;
        Ok(
            serde_json::from_str(&body).unwrap_or(crate::types::GlobalRequestResult {
                success_requests: None,
                failed_requests: None,
            }),
        )
    }

    /// `GET /admin/realms/{realm}/clients/{id}/installation/providers/{providerId}` — get installation config.
    pub async fn client_installation(
        &self,
        realm: &str,
        id: &str,
        provider_id: &str,
    ) -> Result<String> {
        self.get_raw(&format!(
            "{}/{}/installation/providers/{}",
            client_base(realm),
            self.encode_path(id),
            self.encode_path(provider_id)
        ))
        .await
    }

    /// `GET /admin/realms/{realm}/clients/{id}/evaluate-scopes/generate-example-access-token` — generate example token.
    pub async fn client_generate_example_access_token(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<serde_json::Value> {
        self.get(&format!(
            "{}/{}/evaluate-scopes/generate-example-access-token",
            client_base(realm),
            self.encode_path(id)
        ))
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_minimal_client() {
        let json = r#"{"id":"uuid","clientId":"my-client","enabled":true,"publicClient":false,"standardFlowEnabled":true}"#;
        let c: ClientRepresentation = serde_json::from_str(json).unwrap();
        assert_eq!(c.id.as_deref(), Some("uuid"));
        assert_eq!(c.client_id.as_deref(), Some("my-client"));
        assert_eq!(c.enabled, Some(true));
        assert_eq!(c.public_client, Some(false));
        assert_eq!(c.standard_flow_enabled, Some(true));
    }

    #[test]
    fn serialize_client_skips_none() {
        let c = ClientRepresentation {
            client_id: Some("test".into()),
            enabled: Some(true),
            ..Default::default()
        };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"clientId\":\"test\""));
        assert!(!json.contains("redirectUris"));
    }
}
