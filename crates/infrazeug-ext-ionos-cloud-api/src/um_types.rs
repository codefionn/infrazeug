//! Shared types for user management (`/um/*`).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User resource returned by the Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<UserMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<UserProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// User metadata (includes `lastLogin`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_by_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_login: Option<String>,
}

/// User properties (read model).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firstname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub administrator: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_sec_auth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sec_auth_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s3_canonical_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
}

/// Payload for creating a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserCreate {
    pub properties: UserCreateProperties,
}

/// Properties for creating a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserCreateProperties {
    pub firstname: String,
    pub lastname: String,
    pub email: String,
    pub password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub administrator: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_sec_auth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sec_auth_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
}

/// Payload for updating a user.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub properties: UserUpdateProperties,
}

/// Properties for updating a user.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firstname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub administrator: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_sec_auth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sec_auth_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
}

/// Group resource returned by the Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<GroupProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Group properties and contract-level privileges.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GroupProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sec_auth_protection: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_datacenter: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_snapshot: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserve_ip: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_activity_log: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_pcc: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s3_privilege: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_backup_unit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_internet_access: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_k8s_cluster: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_flow_log: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_and_manage_monitoring: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_and_manage_certificates: Option<bool>,
    #[serde(rename = "manageDBaaS", skip_serializing_if = "Option::is_none")]
    pub manage_dbaas: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_and_manage_dns: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manage_registry: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manage_dataplatform: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_and_manage_logging: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_and_manage_cdn: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_and_manage_vpn: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_and_manage_api_gateway: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_and_manage_kaas: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_and_manage_network_file_storage: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_and_manage_ai_model_hub: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_and_manage_iam_resources: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_network_security_groups: Option<bool>,
}

/// Payload for creating a group.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GroupCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    pub properties: GroupProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Payload for updating a group.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GroupUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    pub properties: GroupProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// A resource shared with a group.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GroupShare {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<GroupShareProperties>,
}

/// Share privilege flags.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GroupShareProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_privilege: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share_privilege: Option<bool>,
}

/// Payload for updating group share privileges.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GroupShareUpdate {
    pub properties: GroupShareProperties,
}

/// Reference to a user when adding group membership.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMemberRef {
    pub id: String,
}

/// UM resource entry (datacenter, snapshot, image, …).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UmResource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<crate::types::ElementMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Resource type segment for `/um/resources/{type}`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UmResourceType {
    Datacenter,
    Snapshot,
    Image,
    Ipblock,
    Pcc,
    Backupunit,
    #[serde(rename = "k8s")]
    K8s,
}

/// User Object Storage key.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct S3Key {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<crate::types::ElementMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<S3KeyProperties>,
}

/// Object Storage key properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct S3KeyProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
}

/// Payload for updating an Object Storage key.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct S3KeyUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<crate::types::ElementMetadata>,
    pub properties: S3KeyProperties,
}

/// Object Storage SSO URL response.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct S3SsoUrl {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sso_url: Option<String>,
}
