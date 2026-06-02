//! Identity provider management (`/admin/realms/{realm}/identity-provider`).
//!
//! CRUD for identity providers and their mappers, plus import/export and
//! management permissions.

use crate::client::KeycloakClient;
use crate::error::Result;
use crate::types::{IdentityProviderMapperRepresentation, ManagementPermissionReference};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IdentityProviderRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_profile_first_login_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_email: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_token: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_read_token_role_on_create: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authenticate_by_default: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_on_login: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_broker_login_flow_alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_broker_login_flow_alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_profile_first_login: Option<bool>,
}

fn idp_base(realm: &str) -> String {
    format!("/{}/identity-provider", urlencoding::encode(realm))
}

impl KeycloakClient {
    /// `GET /admin/realms/{realm}/identity-provider/instances` — list identity providers.
    pub async fn identity_providers(
        &self,
        realm: &str,
    ) -> Result<Vec<IdentityProviderRepresentation>> {
        self.get(&format!("{}/instances", idp_base(realm))).await
    }

    /// `GET /admin/realms/{realm}/identity-provider/instances/{alias}` — get a provider.
    pub async fn identity_provider(
        &self,
        realm: &str,
        alias: &str,
    ) -> Result<IdentityProviderRepresentation> {
        self.get(&format!(
            "{}/instances/{}",
            idp_base(realm),
            self.encode_path(alias)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/identity-provider/instances` — create a provider.
    pub async fn create_identity_provider(
        &self,
        realm: &str,
        provider: &IdentityProviderRepresentation,
    ) -> Result<()> {
        self.post(&format!("{}/instances", idp_base(realm)), provider)
            .await
    }

    /// `PUT /admin/realms/{realm}/identity-provider/instances/{alias}` — update a provider.
    pub async fn update_identity_provider(
        &self,
        realm: &str,
        alias: &str,
        provider: &IdentityProviderRepresentation,
    ) -> Result<()> {
        self.put(
            &format!("{}/instances/{}", idp_base(realm), self.encode_path(alias)),
            provider,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/identity-provider/instances/{alias}` — delete a provider.
    pub async fn delete_identity_provider(&self, realm: &str, alias: &str) -> Result<()> {
        self.delete(&format!(
            "{}/instances/{}",
            idp_base(realm),
            self.encode_path(alias)
        ))
        .await
    }

    // ── Mappers ──────────────────────────────────────────────────────

    /// `GET /admin/realms/{realm}/identity-provider/instances/{alias}/mappers` — list mappers.
    pub async fn identity_provider_mappers(
        &self,
        realm: &str,
        alias: &str,
    ) -> Result<Vec<IdentityProviderMapperRepresentation>> {
        self.get(&format!(
            "{}/instances/{}/mappers",
            idp_base(realm),
            self.encode_path(alias)
        ))
        .await
    }

    /// `POST /admin/realms/{realm}/identity-provider/instances/{alias}/mappers` — add a mapper.
    pub async fn identity_provider_add_mapper(
        &self,
        realm: &str,
        alias: &str,
        mapper: &IdentityProviderMapperRepresentation,
    ) -> Result<()> {
        self.post(
            &format!(
                "{}/instances/{}/mappers",
                idp_base(realm),
                self.encode_path(alias)
            ),
            mapper,
        )
        .await
    }

    /// `DELETE /admin/realms/{realm}/identity-provider/instances/{alias}/mappers/{id}` — delete a mapper.
    pub async fn identity_provider_delete_mapper(
        &self,
        realm: &str,
        alias: &str,
        mapper_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "{}/instances/{}/mappers/{}",
            idp_base(realm),
            self.encode_path(alias),
            self.encode_path(mapper_id)
        ))
        .await
    }

    /// `PUT /admin/realms/{realm}/identity-provider/instances/{alias}/mappers/{id}` — update a mapper.
    pub async fn identity_provider_update_mapper(
        &self,
        realm: &str,
        alias: &str,
        mapper_id: &str,
        mapper: &IdentityProviderMapperRepresentation,
    ) -> Result<()> {
        self.put(
            &format!(
                "{}/instances/{}/mappers/{}",
                idp_base(realm),
                self.encode_path(alias),
                self.encode_path(mapper_id)
            ),
            mapper,
        )
        .await
    }

    // ── Management permissions ───────────────────────────────────────

    /// `GET /admin/realms/{realm}/identity-provider/instances/{alias}/management/permissions` — get permissions.
    pub async fn identity_provider_management_permissions(
        &self,
        realm: &str,
        alias: &str,
    ) -> Result<ManagementPermissionReference> {
        self.get(&format!(
            "{}/instances/{}/management/permissions",
            idp_base(realm),
            self.encode_path(alias)
        ))
        .await
    }

    /// `PUT /admin/realms/{realm}/identity-provider/instances/{alias}/management/permissions` — update permissions.
    pub async fn identity_provider_update_management_permissions(
        &self,
        realm: &str,
        alias: &str,
        perm: &ManagementPermissionReference,
    ) -> Result<ManagementPermissionReference> {
        self.put_json(
            &format!(
                "{}/instances/{}/management/permissions",
                idp_base(realm),
                self.encode_path(alias)
            ),
            perm,
        )
        .await
    }

    // ── Import / Export ──────────────────────────────────────────────

    /// `POST /admin/realms/{realm}/identity-provider/import-config` — import provider config.
    pub async fn identity_provider_import_config(
        &self,
        realm: &str,
        body: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        self.put_json(&format!("{}/import-config", idp_base(realm)), body)
            .await
    }

    /// `GET /admin/realms/{realm}/identity-provider/instances/{alias}/export` — export provider config.
    pub async fn identity_provider_export(&self, realm: &str, alias: &str) -> Result<String> {
        self.get_raw(&format!(
            "{}/instances/{}/export",
            idp_base(realm),
            self.encode_path(alias)
        ))
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_identity_provider() {
        let json = r#"{
            "alias":"google",
            "displayName":"Google",
            "providerId":"google",
            "enabled":true,
            "trustEmail":true,
            "storeToken":false,
            "config":{"clientId":"xxx","clientSecret":"yyy"}
        }"#;
        let p: IdentityProviderRepresentation = serde_json::from_str(json).unwrap();
        assert_eq!(p.alias.as_deref(), Some("google"));
        assert_eq!(p.provider_id.as_deref(), Some("google"));
        assert_eq!(p.enabled, Some(true));
        assert_eq!(
            p.config.unwrap().get("clientId").map(String::as_str),
            Some("xxx")
        );
    }

    #[test]
    fn serialize_identity_provider() {
        let p = IdentityProviderRepresentation {
            alias: Some("oidc".into()),
            provider_id: Some("oidc".into()),
            enabled: Some(true),
            ..Default::default()
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"alias\":\"oidc\""));
        assert!(!json.contains("displayName"));
    }
}
