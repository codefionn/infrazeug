//! Shared Keycloak Admin API types.
//!
//! These types mirror the JSON representations defined in the Keycloak Admin
//! REST API. Field naming follows `camelCase` as used on the wire. Optional
//! fields are `Option<T>` and default to `None`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use super::client_scopes::ClientScopeRepresentation;
pub use super::clients::ClientRepresentation;
pub use super::components::ComponentRepresentation;
pub use super::groups::GroupRepresentation;
pub use super::identity_providers::IdentityProviderRepresentation;
pub use super::realms::RealmRepresentation;
pub use super::roles::RoleRepresentation;
pub use super::users::UserRepresentation;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_date: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temporary: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolMapperRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_mapper: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent_required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FederatedIdentityRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserConsentRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_date: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granted_client_scopes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated_date: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSessionRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_access: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clients: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagementPermissionReference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_permissions: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolesRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm: Option<Vec<RoleRepresentation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<HashMap<String, Vec<RoleRepresentation>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingsRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm_mappings: Option<Vec<RoleRepresentation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_mappings: Option<HashMap<String, ClientMappingsRepresentation>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientMappingsRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mappings: Option<Vec<RoleRepresentation>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificateRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyStoreConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm_certificate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm_key: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityProviderMapperRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_provider_alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_provider_mapper: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRepresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RealmEventsConfigRepresentation {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalRequestResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_requests: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_requests: Option<Vec<String>>,
}
