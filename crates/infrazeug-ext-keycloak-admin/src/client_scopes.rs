//! Client scope management (`/admin/realms/{realm}/client-scopes`).
//!
//! CRUD for client scopes and protocol mappers within scopes.

use crate::client::KeycloakClient;
use crate::error::Result;
use crate::types::ProtocolMapperRepresentation;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientScopeRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_mappers: Option<Vec<ProtocolMapperRepresentation>>,
}

fn scopes_base(realm: &str) -> String {
    format!("/{}/client-scopes", urlencoding::encode(realm))
}

impl KeycloakClient {
    /// `GET /admin/realms/{realm}/client-scopes` — list all client scopes.
    pub async fn client_scopes(&self, realm: &str) -> Result<Vec<ClientScopeRepresentation>> {
        self.get(&scopes_base(realm)).await
    }

    /// `POST /admin/realms/{realm}/client-scopes` — create a client scope. Returns the new scope ID.
    pub async fn create_client_scope(
        &self,
        realm: &str,
        scope: &ClientScopeRepresentation,
    ) -> Result<String> {
        self.post_and_extract_id(&scopes_base(realm), scope).await
    }

    /// `GET /admin/realms/{realm}/client-scopes/{id}` — get a client scope.
    pub async fn client_scope(&self, realm: &str, id: &str) -> Result<ClientScopeRepresentation> {
        self.get(&format!("{}/{}", scopes_base(realm), self.encode_path(id)))
            .await
    }

    /// `PUT /admin/realms/{realm}/client-scopes/{id}` — update a client scope.
    pub async fn update_client_scope(
        &self,
        realm: &str,
        id: &str,
        scope: &ClientScopeRepresentation,
    ) -> Result<()> {
        self.put(
            &format!("{}/{}", scopes_base(realm), self.encode_path(id)),
            scope,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/client-scopes/{id}` — delete a client scope.
    pub async fn delete_client_scope(&self, realm: &str, id: &str) -> Result<()> {
        self.delete(&format!("{}/{}", scopes_base(realm), self.encode_path(id)))
            .await
    }

    // ── Protocol mappers within client scope ─────────────────────────

    /// `GET /admin/realms/{realm}/client-scopes/{id}/protocol-mappers` — list protocol mappers.
    pub async fn client_scope_protocol_mappers(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<Vec<ProtocolMapperRepresentation>> {
        self.get(&format!(
            "{}/{}/protocol-mappers/models",
            scopes_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/client-scopes/{id}/protocol-mappers/models` — add a protocol mapper.
    pub async fn client_scope_add_protocol_mapper(
        &self,
        realm: &str,
        id: &str,
        mapper: &ProtocolMapperRepresentation,
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/protocol-mappers/models",
                scopes_base(realm),
                self.encode_path(id)
            ),
            mapper,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/client-scopes/{id}/protocol-mappers/models/{mapperId}` — delete mapper.
    pub async fn client_scope_delete_protocol_mapper(
        &self,
        realm: &str,
        id: &str,
        mapper_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "{}/{}/protocol-mappers/models/{}",
            scopes_base(realm),
            self.encode_path(id),
            self.encode_path(mapper_id)
        ))
        .await
    }

    /// `PUT /admin/realms/{realm}/client-scopes/{id}/protocol-mappers/models/{mapperId}` — update mapper.
    pub async fn client_scope_update_protocol_mapper(
        &self,
        realm: &str,
        id: &str,
        mapper_id: &str,
        mapper: &ProtocolMapperRepresentation,
    ) -> Result<()> {
        self.put(
            &format!(
                "{}/{}/protocol-mappers/models/{}",
                scopes_base(realm),
                self.encode_path(id),
                self.encode_path(mapper_id)
            ),
            mapper,
        )
        .await
    }

    // ── Scope mappings ───────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/client-scopes/{id}/scope-mappings` — get scope mappings.
    pub async fn client_scope_scope_mappings(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<crate::types::MappingsRepresentation> {
        self.get(&format!(
            "{}/{}/scope-mappings",
            scopes_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/client-scopes/{id}/scope-mappings/realm` — add realm scope mappings.
    pub async fn client_scope_add_realm_scope_mappings(
        &self,
        realm: &str,
        id: &str,
        roles: &[crate::roles::RoleRepresentation],
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/scope-mappings/realm",
                scopes_base(realm),
                self.encode_path(id)
            ),
            roles,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_client_scope() {
        let json = r#"{
            "id":"scope-1",
            "name":"email",
            "description":"OpenID Connect built-in scope: email",
            "protocol":"openid-connect",
            "attributes":{"consent.screen.text":"${emailScopeConsentText}","display.on.consent.screen":"true"}
        }"#;
        let s: ClientScopeRepresentation = serde_json::from_str(json).unwrap();
        assert_eq!(s.id.as_deref(), Some("scope-1"));
        assert_eq!(s.name.as_deref(), Some("email"));
        assert_eq!(s.protocol.as_deref(), Some("openid-connect"));
    }

    #[test]
    fn serialize_client_scope() {
        let s = ClientScopeRepresentation {
            name: Some("profile".into()),
            protocol: Some("openid-connect".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"name\":\"profile\""));
        assert!(!json.contains("description"));
    }
}
