//! Group management (`/admin/realms/{realm}/groups`).
//!
//! CRUD for groups and sub-groups, role mappings, member listing, and
//! management permissions.

use crate::client::KeycloakClient;
use crate::error::Result;
use crate::types::{ManagementPermissionReference, MappingsRepresentation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GroupRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_group_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub_groups: Vec<GroupRepresentation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm_roles: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_roles: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<HashMap<String, bool>>,
}

fn groups_base(realm: &str) -> String {
    format!("/{}/groups", urlencoding::encode(realm))
}

impl KeycloakClient {
    /// `GET /admin/realms/{realm}/groups` — list top-level groups.
    pub async fn groups(&self, realm: &str) -> Result<Vec<GroupRepresentation>> {
        self.get(&groups_base(realm)).await
    }

    /// `GET /admin/realms/{realm}/groups?search=...` — search groups.
    pub async fn groups_search(
        &self,
        realm: &str,
        search: &str,
    ) -> Result<Vec<GroupRepresentation>> {
        self.get_with_query(&groups_base(realm), &[("search", search)])
            .await
    }

    /// `GET /admin/realms/{realm}/groups/{id}` — get a single group.
    pub async fn group(&self, realm: &str, id: &str) -> Result<GroupRepresentation> {
        self.get(&format!("{}/{}", groups_base(realm), self.encode_path(id)))
            .await
    }

    /// `POST /admin/realms/{realm}/groups` — create a top-level group. Returns the new group ID.
    pub async fn create_group(&self, realm: &str, group: &GroupRepresentation) -> Result<String> {
        self.post_and_extract_id(&groups_base(realm), group).await
    }

    /// `PUT /admin/realms/{realm}/groups/{id}` — update a group.
    pub async fn update_group(
        &self,
        realm: &str,
        id: &str,
        group: &GroupRepresentation,
    ) -> Result<()> {
        self.put(
            &format!("{}/{}", groups_base(realm), self.encode_path(id)),
            group,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/groups/{id}` — delete a group.
    pub async fn delete_group(&self, realm: &str, id: &str) -> Result<()> {
        self.delete(&format!("{}/{}", groups_base(realm), self.encode_path(id)))
            .await
    }

    // ── Sub-groups ───────────────────────────────────────────────────

    /// `POST /admin/realms/{realm}/groups/{id}/children` — create a sub-group.
    pub async fn create_sub_group(
        &self,
        realm: &str,
        parent_id: &str,
        group: &GroupRepresentation,
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/children",
                groups_base(realm),
                self.encode_path(parent_id)
            ),
            group,
        )
        .await
    }

    // ── Members ──────────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/groups/{id}/members` — list group members.
    pub async fn group_members(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<Vec<crate::users::UserRepresentation>> {
        self.get(&format!(
            "{}/{}/members",
            groups_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `GET /admin/realms/{realm}/groups/{id}/members?...` — list group members with query.
    pub async fn group_members_with_query(
        &self,
        realm: &str,
        id: &str,
        query: &[(&str, &str)],
    ) -> Result<Vec<crate::users::UserRepresentation>> {
        self.get_with_query(
            &format!("{}/{}/members", groups_base(realm), self.encode_path(id)),
            query,
        )
        .await
    }

    // ── Role mappings ────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/groups/{id}/role-mappings` — get all role mappings.
    pub async fn group_role_mappings(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<MappingsRepresentation> {
        self.get(&format!(
            "{}/{}/role-mappings",
            groups_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/groups/{id}/role-mappings/realm` — add realm role mappings.
    pub async fn group_add_realm_roles(
        &self,
        realm: &str,
        id: &str,
        roles: &[crate::roles::RoleRepresentation],
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/role-mappings/realm",
                groups_base(realm),
                self.encode_path(id)
            ),
            roles,
        )
        .await
    }

    /// `GET /admin/realms/{realm}/groups/{id}/role-mappings/realm` — get realm role mappings.
    pub async fn group_realm_roles(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<Vec<crate::roles::RoleRepresentation>> {
        self.get(&format!(
            "{}/{}/role-mappings/realm",
            groups_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `DELETE /admin/realms/{realm}/groups/{id}/role-mappings/realm` — remove realm role mappings.
    pub async fn group_remove_realm_roles(
        &self,
        realm: &str,
        id: &str,
        roles: &[crate::roles::RoleRepresentation],
    ) -> Result<()> {
        let body = serde_json::to_string(roles)?;
        self.request_raw(
            reqwest::Method::DELETE,
            &format!(
                "{}/{}/role-mappings/realm",
                groups_base(realm),
                self.encode_path(id)
            ),
            Some(&body),
        )
        .await?;
        Ok(())
    }

    /// `POST /admin/realms/{realm}/groups/{id}/role-mappings/clients/{client}` — add client role mappings.
    pub async fn group_add_client_roles(
        &self,
        realm: &str,
        id: &str,
        client_id: &str,
        roles: &[crate::roles::RoleRepresentation],
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/{}/role-mappings/clients/{}",
                groups_base(realm),
                self.encode_path(id),
                self.encode_path(client_id)
            ),
            roles,
        )
        .await
    }

    /// `GET /admin/realms/{realm}/groups/{id}/role-mappings/clients/{client}` — get client role mappings.
    pub async fn group_client_roles(
        &self,
        realm: &str,
        id: &str,
        client_id: &str,
    ) -> Result<Vec<crate::roles::RoleRepresentation>> {
        self.get(&format!(
            "{}/{}/role-mappings/clients/{}",
            groups_base(realm),
            self.encode_path(id),
            self.encode_path(client_id)
        ))
        .await
    }

    // ── Management permissions ───────────────────────────────────────

    /// `GET /admin/realms/{realm}/groups/{id}/management/permissions` — get management permissions.
    pub async fn group_management_permissions(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<ManagementPermissionReference> {
        self.get(&format!(
            "{}/{}/management/permissions",
            groups_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `PUT /admin/realms/{realm}/groups/{id}/management/permissions` — update management permissions.
    pub async fn group_update_management_permissions(
        &self,
        realm: &str,
        id: &str,
        perm: &ManagementPermissionReference,
    ) -> Result<ManagementPermissionReference> {
        self.put_json(
            &format!(
                "{}/{}/management/permissions",
                groups_base(realm),
                self.encode_path(id)
            ),
            perm,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_group() {
        let json = r#"{
            "id":"g1","name":"admins","path":"/admins",
            "subGroups":[{"id":"g2","name":"super-admins","path":"/admins/super-admins","subGroups":[]}],
            "realmRoles":["admin"],
            "attributes":{"priority":["high"]}
        }"#;
        let g: GroupRepresentation = serde_json::from_str(json).unwrap();
        assert_eq!(g.id.as_deref(), Some("g1"));
        assert_eq!(g.name.as_deref(), Some("admins"));
        assert_eq!(g.path.as_deref(), Some("/admins"));
        assert_eq!(g.sub_groups.len(), 1);
        assert_eq!(g.realm_roles.as_ref().unwrap().len(), 1);
        assert_eq!(g.attributes.unwrap().get("priority").unwrap()[0], "high");
    }

    #[test]
    fn serialize_group_skips_none() {
        let g = GroupRepresentation {
            name: Some("users".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&g).unwrap();
        assert!(json.contains("\"name\":\"users\""));
        assert!(!json.contains("subGroups"));
    }
}
