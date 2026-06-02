//! Component management (`/admin/realms/{realm}/components`).
//!
//! CRUD for realm components such as key providers, user federation providers,
//! and other extension points.

use crate::client::KeycloakClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ComponentRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, Vec<String>>>,
}

fn components_base(realm: &str) -> String {
    format!("/{}/components", urlencoding::encode(realm))
}

impl KeycloakClient {
    /// `GET /admin/realms/{realm}/components` — list all components.
    pub async fn components(&self, realm: &str) -> Result<Vec<ComponentRepresentation>> {
        self.get(&components_base(realm)).await
    }

    /// `GET /admin/realms/{realm}/components?parent=...&type=...` — search components.
    pub async fn components_search(
        &self,
        realm: &str,
        query: &[(&str, &str)],
    ) -> Result<Vec<ComponentRepresentation>> {
        self.get_with_query(&components_base(realm), query).await
    }

    /// `GET /admin/realms/{realm}/components/{id}` — get a component.
    pub async fn component(&self, realm: &str, id: &str) -> Result<ComponentRepresentation> {
        self.get(&format!(
            "{}/{}",
            components_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/components` — create a component. Returns the new component ID.
    pub async fn create_component(
        &self,
        realm: &str,
        component: &ComponentRepresentation,
    ) -> Result<String> {
        self.post_and_extract_id(&components_base(realm), component)
            .await
    }

    /// `PUT /admin/realms/{realm}/components/{id}` — update a component.
    pub async fn update_component(
        &self,
        realm: &str,
        id: &str,
        component: &ComponentRepresentation,
    ) -> Result<()> {
        self.put(
            &format!("{}/{}", components_base(realm), self.encode_path(id)),
            component,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/components/{id}` — delete a component.
    pub async fn delete_component(&self, realm: &str, id: &str) -> Result<()> {
        self.delete(&format!(
            "{}/{}",
            components_base(realm),
            self.encode_path(id)
        ))
        .await
    }

    /// `GET /admin/realms/{realm}/components/{id}/sub-component-types` — get sub-component types.
    pub async fn component_sub_component_types(
        &self,
        realm: &str,
        id: &str,
    ) -> Result<Vec<serde_json::Value>> {
        self.get(&format!(
            "{}/{}/sub-component-types",
            components_base(realm),
            self.encode_path(id)
        ))
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_component() {
        let json = r#"{
            "id":"comp-1",
            "name":"rsa-generated",
            "providerId":"rsa-generated",
            "providerType":"org.keycloak.keys.KeyProvider",
            "parentId":"realm-id",
            "config":{"priority":["100"],"algorithm":["RS256"]}
        }"#;
        let c: ComponentRepresentation = serde_json::from_str(json).unwrap();
        assert_eq!(c.id.as_deref(), Some("comp-1"));
        assert_eq!(c.name.as_deref(), Some("rsa-generated"));
        assert_eq!(
            c.provider_type.as_deref(),
            Some("org.keycloak.keys.KeyProvider")
        );
        let config = c.config.unwrap();
        assert_eq!(config.get("priority").unwrap()[0], "100");
    }

    #[test]
    fn serialize_component() {
        let c = ComponentRepresentation {
            name: Some("my-key".into()),
            provider_id: Some("rsa".into()),
            provider_type: Some("org.keycloak.keys.KeyProvider".into()),
            parent_id: Some("realm-id".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"name\":\"my-key\""));
        assert!(!json.contains("subType"));
    }
}
