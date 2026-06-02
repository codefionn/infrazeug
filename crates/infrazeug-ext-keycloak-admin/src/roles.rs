//! Role management (`/admin/realms/{realm}/roles`).
//!
//! Realm-level role CRUD and composite management. Client-level roles are
//! accessible via `/admin/realms/{realm}/clients/{id}/roles`.

use crate::client::KeycloakClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RoleRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_param_required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composite: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composites: Option<Composites>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_role: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Composites {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<HashMap<String, Vec<String>>>,
}

fn roles_base(realm: &str) -> String {
    format!("/{}/roles", urlencoding::encode(realm))
}

fn client_roles_base(realm: &str, client_id: &str) -> String {
    format!(
        "/{}/clients/{}/roles",
        urlencoding::encode(realm),
        urlencoding::encode(client_id)
    )
}

impl KeycloakClient {
    // ── Realm roles ──────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/roles` — list all realm roles.
    pub async fn roles(&self, realm: &str) -> Result<Vec<RoleRepresentation>> {
        self.get(&roles_base(realm)).await
    }

    /// `GET /admin/realms/{realm}/roles/{name}` — get a realm role by name.
    pub async fn role(&self, realm: &str, name: &str) -> Result<RoleRepresentation> {
        self.get(&format!("{}/{}", roles_base(realm), self.encode_path(name)))
            .await
    }

    /// `POST /admin/realms/{realm}/roles` — create a realm role.
    pub async fn create_role(&self, realm: &str, role: &RoleRepresentation) -> Result<()> {
        self.post(&roles_base(realm), role).await
    }

    /// `PUT /admin/realms/{realm}/roles/{name}` — update a realm role.
    pub async fn update_role(
        &self,
        realm: &str,
        name: &str,
        role: &RoleRepresentation,
    ) -> Result<()> {
        self.put(
            &format!("{}/{}", roles_base(realm), self.encode_path(name)),
            role,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/roles/{name}` — delete a realm role.
    pub async fn delete_role(&self, realm: &str, name: &str) -> Result<()> {
        self.delete(&format!("{}/{}", roles_base(realm), self.encode_path(name)))
            .await
    }

    /// `GET /admin/realms/{realm}/roles/{name}/composites` — get composite roles.
    pub async fn role_composites(
        &self,
        realm: &str,
        name: &str,
    ) -> Result<Vec<RoleRepresentation>> {
        self.get(&format!(
            "{}/{}/composites",
            roles_base(realm),
            self.encode_path(name)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/roles/{name}/composites` — add composite roles.
    pub async fn role_add_composites(
        &self,
        realm: &str,
        name: &str,
        roles: &[RoleRepresentation],
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/composites",
                roles_base(realm),
                self.encode_path(name)
            ),
            roles,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/roles/{name}/composites` — remove composite roles.
    pub async fn role_remove_composites(
        &self,
        realm: &str,
        name: &str,
        roles: &[RoleRepresentation],
    ) -> Result<()> {
        let body = serde_json::to_string(roles)?;
        self.request_raw(
            reqwest::Method::DELETE,
            &format!(
                "{}/{}/composites",
                roles_base(realm),
                self.encode_path(name)
            ),
            Some(&body),
        )
        .await?;
        Ok(())
    }

    /// `GET /admin/realms/{realm}/roles?search=...` — search realm roles.
    pub async fn roles_search(&self, realm: &str, search: &str) -> Result<Vec<RoleRepresentation>> {
        self.get_with_query(&roles_base(realm), &[("search", search)])
            .await
    }

    // ── Client roles ─────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/clients/{id}/roles` — list client roles.
    pub async fn client_roles(
        &self,
        realm: &str,
        client_id: &str,
    ) -> Result<Vec<RoleRepresentation>> {
        self.get(&client_roles_base(realm, client_id)).await
    }

    /// `GET /admin/realms/{realm}/clients/{id}/roles/{name}` — get a client role.
    pub async fn client_role(
        &self,
        realm: &str,
        client_id: &str,
        name: &str,
    ) -> Result<RoleRepresentation> {
        self.get(&format!(
            "{}/{}",
            client_roles_base(realm, client_id),
            self.encode_path(name)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/clients/{id}/roles` — create a client role.
    pub async fn create_client_role(
        &self,
        realm: &str,
        client_id: &str,
        role: &RoleRepresentation,
    ) -> Result<()> {
        self.post(&client_roles_base(realm, client_id), role).await
    }

    /// `PUT /admin/realms/{realm}/clients/{id}/roles/{name}` — update a client role.
    pub async fn update_client_role(
        &self,
        realm: &str,
        client_id: &str,
        name: &str,
        role: &RoleRepresentation,
    ) -> Result<()> {
        self.put(
            &format!(
                "{}/{}",
                client_roles_base(realm, client_id),
                self.encode_path(name)
            ),
            role,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/clients/{id}/roles/{name}` — delete a client role.
    pub async fn delete_client_role(&self, realm: &str, client_id: &str, name: &str) -> Result<()> {
        self.delete(&format!(
            "{}/{}",
            client_roles_base(realm, client_id),
            self.encode_path(name)
        ))
        .await
    }

    /// `GET /admin/realms/{realm}/clients/{id}/roles/{name}/composites` — get client role composites.
    pub async fn client_role_composites(
        &self,
        realm: &str,
        client_id: &str,
        name: &str,
    ) -> Result<Vec<RoleRepresentation>> {
        self.get(&format!(
            "{}/{}/composites",
            client_roles_base(realm, client_id),
            self.encode_path(name)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/clients/{id}/roles/{name}/composites` — add client role composites.
    pub async fn client_role_add_composites(
        &self,
        realm: &str,
        client_id: &str,
        name: &str,
        roles: &[RoleRepresentation],
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/composites",
                client_roles_base(realm, client_id),
                self.encode_path(name)
            ),
            roles,
        )
        .await
    }

    // ── Roles by ID ──────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/roles-by-id/{id}` — get role by ID.
    pub async fn role_by_id(&self, realm: &str, id: &str) -> Result<RoleRepresentation> {
        self.get(&format!(
            "/{}/roles-by-id/{}",
            urlencoding::encode(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `GET /admin/realms/{realm}/roles-by-id/{id}/composites` — get composites by role ID.
    pub async fn role_by_id_composites(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<Vec<RoleRepresentation>> {
        self.get(&format!(
            "/{}/roles-by-id/{}/composites",
            urlencoding::encode(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/roles-by-id/{id}/composites` — add composites by role ID.
    pub async fn role_by_id_add_composites(
        &self,
        realm: &str,
        id: &str,
        roles: &[RoleRepresentation],
    ) -> Result<()> {
        self.post(
            &format!(
                "/{}/roles-by-id/{}/composites",
                urlencoding::encode(realm),
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
    fn deserialize_role() {
        let json = r#"{"id":"role-id","name":"admin","composite":true,"clientRole":false,"containerId":"realm-id"}"#;
        let r: RoleRepresentation = serde_json::from_str(json).unwrap();
        assert_eq!(r.name.as_deref(), Some("admin"));
        assert_eq!(r.composite, Some(true));
        assert_eq!(r.client_role, Some(false));
    }

    #[test]
    fn deserialize_role_with_composites() {
        let json = r#"{
            "id":"r1","name":"super-admin","composite":true,
            "composites":{"realm":["admin"],"client":{"my-client":["client-admin"]}}
        }"#;
        let r: RoleRepresentation = serde_json::from_str(json).unwrap();
        let c = r.composites.unwrap();
        assert_eq!(c.realm.unwrap(), vec!["admin"]);
        assert_eq!(c.client.unwrap().get("my-client").unwrap().len(), 1);
    }

    #[test]
    fn serialize_role_skips_none() {
        let r = RoleRepresentation {
            name: Some("user".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"name\":\"user\""));
        assert!(!json.contains("composite"));
    }
}
