//! OVHcloud API v2 **okms** bindings (`/v2/okms`).
//!
//! Generated from the official schema; do not edit by hand.

#![allow(unused_imports, unused_variables)]

use crate::client::{percent_encode, OvhClient, PageParams, V2RequestOptions};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// `dbaas.logs.LogKind`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsLogKind {
    pub additional_returned_fields: Option<Vec<String>>,
    pub created_at: Option<String>,
    pub display_name: Option<String>,
    pub kind_id: Option<String>,
    pub name: Option<String>,
    pub updated_at: Option<String>,
}

/// `dbaas.logs.LogSubscriptionResource`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsLogSubscriptionResource {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

/// `dbaas.logs.LogSubscription`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsLogSubscription {
    pub created_at: Option<String>,
    pub kind: Option<String>,
    pub resource: Option<LogsLogSubscriptionResource>,
    pub service_name: Option<String>,
    pub stream_id: Option<String>,
    pub subscription_id: Option<String>,
    pub updated_at: Option<String>,
}

/// `dbaas.logs.LogSubscriptionCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsLogSubscriptionCreation {
    pub kind: String,
    pub stream_id: String,
}

/// `dbaas.logs.LogSubscriptionResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsLogSubscriptionResponse {
    pub operation_id: Option<String>,
    pub service_name: Option<String>,
}

/// `dbaas.logs.LogUrlCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsLogUrlCreation {
    pub kind: String,
}

/// `dbaas.logs.TemporaryLogsLink`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsTemporaryLogsLink {
    pub expiration_date: Option<String>,
    pub url: Option<String>,
}

/// `iam.resource.TagFilter.OperatorEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IamResourceTagFilterOperator {
    #[serde(rename = "EQ")]
    Eq,
    #[serde(rename = "EXISTS")]
    Exists,
    #[serde(rename = "ILIKE")]
    Ilike,
    #[serde(rename = "LIKE")]
    Like,
    #[serde(rename = "NEQ")]
    Neq,
    #[serde(rename = "NEXISTS")]
    Nexists,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `iam.resource.TagFilter`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTagFilter {
    pub operator: Option<IamResourceTagFilterOperator>,
    pub value: Option<String>,
}

/// `location.TypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocationType {
    #[serde(rename = "LOCAL-ZONE")]
    LocalZone,
    #[serde(rename = "REGION-1-AZ")]
    Region1Az,
    #[serde(rename = "REGION-3-AZ")]
    Region3Az,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `observability.CertificationLevelEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ObservabilityCertificationLevel {
    #[serde(rename = "HDS")]
    Hds,
    #[serde(rename = "PCI_DSS")]
    PciDss,
    #[serde(rename = "SNC")]
    Snc,
    #[serde(rename = "SOC2")]
    Soc2,
    #[serde(rename = "STANDARD")]
    Standard,
    #[serde(rename = "TRUSTED_ZONE")]
    TrustedZone,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.CertificateTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsCertificateType {
    #[serde(rename = "ECDSA")]
    Ecdsa,
    #[serde(rename = "RSA")]
    Rsa,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.CredentialStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsCredentialStatus {
    #[serde(rename = "CREATING")]
    Creating,
    #[serde(rename = "DELETING")]
    Deleting,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "EXPIRED")]
    Expired,
    #[serde(rename = "READY")]
    Ready,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.KeyAlgEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsKeyAlg {
    #[serde(rename = "ES256")]
    Es256,
    #[serde(rename = "ES384")]
    Es384,
    #[serde(rename = "ES512")]
    Es512,
    #[serde(rename = "PS256")]
    Ps256,
    #[serde(rename = "PS384")]
    Ps384,
    #[serde(rename = "PS512")]
    Ps512,
    #[serde(rename = "RS256")]
    Rs256,
    #[serde(rename = "RS384")]
    Rs384,
    #[serde(rename = "RS512")]
    Rs512,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.KeyCurveEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsKeyCurve {
    #[serde(rename = "P-256")]
    P256,
    #[serde(rename = "P-384")]
    P384,
    #[serde(rename = "P-521")]
    P521,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.KeyDeactivationReasonEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsKeyDeactivationReason {
    #[serde(rename = "AFFILIATION_CHANGED")]
    AffiliationChanged,
    #[serde(rename = "CA_COMPROMISE")]
    CaCompromise,
    #[serde(rename = "CESSATION_OF_OPERATION")]
    CessationOfOperation,
    #[serde(rename = "KEY_COMPROMISE")]
    KeyCompromise,
    #[serde(rename = "PRIVILEGE_WITHDRAWN")]
    PrivilegeWithdrawn,
    #[serde(rename = "SUPERSEDED")]
    Superseded,
    #[serde(rename = "UNSPECIFIED")]
    Unspecified,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.KeyFormatEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsKeyFormat {
    #[serde(rename = "JWK")]
    Jwk,
    #[serde(rename = "PEM")]
    Pem,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.KeyOpsEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsKeyOps {
    #[serde(rename = "decrypt")]
    Decrypt,
    #[serde(rename = "encrypt")]
    Encrypt,
    #[serde(rename = "sign")]
    Sign,
    #[serde(rename = "unwrapKey")]
    UnwrapKey,
    #[serde(rename = "verify")]
    Verify,
    #[serde(rename = "wrapKey")]
    WrapKey,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.KeySizeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsKeySize {
    #[serde(rename = "128")]
    V128,
    #[serde(rename = "192")]
    V192,
    #[serde(rename = "256")]
    V256,
    #[serde(rename = "384")]
    V384,
    #[serde(rename = "521")]
    V521,
    #[serde(rename = "2048")]
    V2048,
    #[serde(rename = "3072")]
    V3072,
    #[serde(rename = "4096")]
    V4096,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.KeyStateEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsKeyState {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "ALL")]
    All,
    #[serde(rename = "COMPROMISED")]
    Compromised,
    #[serde(rename = "DEACTIVATED")]
    Deactivated,
    #[serde(rename = "DESTROYED")]
    Destroyed,
    #[serde(rename = "DESTROYED_COMPROMISED")]
    DestroyedCompromised,
    #[serde(rename = "PRE_ACTIVE")]
    PreActive,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.KeyStateUpdateEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsKeyStateUpdate {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "COMPROMISED")]
    Compromised,
    #[serde(rename = "DEACTIVATED")]
    Deactivated,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.KeyTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsKeyType {
    #[serde(rename = "EC")]
    Ec,
    #[serde(rename = "RSA")]
    Rsa,
    #[serde(rename = "oct")]
    Oct,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.KeyUseEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsKeyUse {
    #[serde(rename = "enc")]
    Enc,
    #[serde(rename = "sig")]
    Sig,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.ProtectionLevelEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsProtectionLevel {
    #[serde(rename = "HSM")]
    Hsm,
    #[serde(rename = "MANAGED_HSM")]
    ManagedHsm,
    #[serde(rename = "SOFTWARE")]
    Software,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.RegionEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsRegion {
    #[serde(rename = "ap-south-mum")]
    ApSouthMum,
    #[serde(rename = "ap-southeast-sgp")]
    ApSoutheastSgp,
    #[serde(rename = "ap-southeast-syd")]
    ApSoutheastSyd,
    #[serde(rename = "ca-east-bhs")]
    CaEastBhs,
    #[serde(rename = "ca-east-tor")]
    CaEastTor,
    #[serde(rename = "eu-central-waw")]
    EuCentralWaw,
    #[serde(rename = "eu-south-mil")]
    EuSouthMil,
    #[serde(rename = "eu-west-eri")]
    EuWestEri,
    #[serde(rename = "eu-west-gra")]
    EuWestGra,
    #[serde(rename = "eu-west-lim")]
    EuWestLim,
    #[serde(rename = "eu-west-par")]
    EuWestPar,
    #[serde(rename = "eu-west-rbx")]
    EuWestRbx,
    #[serde(rename = "eu-west-sbg")]
    EuWestSbg,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.SecretStateEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OkmsSecretState {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "DEACTIVATED")]
    Deactivated,
    #[serde(rename = "DELETED")]
    Deleted,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `okms.credential.Creation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialCreation {
    pub certificate_type: Option<OkmsCertificateType>,
    #[serde(default)]
    pub csr: serde_json::Value,
    pub description: Option<String>,
    #[serde(default)]
    pub identity_urns: Vec<String>,
    pub name: String,
    pub validity: Option<i64>,
}

/// `okms.credential.CreationResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialCreationResponse {
    pub certificate_type: Option<OkmsCertificateType>,
    pub created_at: Option<String>,
    pub description: Option<String>,
    pub expired_at: Option<String>,
    pub from_csr: Option<bool>,
    pub id: Option<String>,
    pub identity_urns: Option<Vec<String>>,
    pub name: Option<String>,
    #[serde(default)]
    pub private_key_pem: serde_json::Value,
    pub status: Option<OkmsCredentialStatus>,
}

/// `okms.credential.GetResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialGetResponse {
    #[serde(default)]
    pub certificate_pem: serde_json::Value,
    pub certificate_type: Option<OkmsCertificateType>,
    pub created_at: Option<String>,
    pub description: Option<String>,
    pub expired_at: Option<String>,
    pub from_csr: Option<bool>,
    pub id: Option<String>,
    pub identity_urns: Option<Vec<String>>,
    pub name: Option<String>,
    pub status: Option<OkmsCredentialStatus>,
}

/// `okms.reference.Region`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceRegion {
    pub certifications: Option<Vec<ObservabilityCertificationLevel>>,
    pub id: Option<OkmsRegion>,
    pub kmip_endpoint: Option<String>,
    pub rest_endpoint: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<LocationType>,
}

/// `okms.reference.secretConfig.Response`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceSecretConfigResponse {
    pub cas_required: Option<bool>,
    pub max_versions: Option<i64>,
}

/// `okms.reference.serviceKey.Curve`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceServiceKeyCurve {
    pub default: Option<bool>,
    pub value: Option<OkmsKeyCurve>,
}

/// `okms.reference.serviceKey.Operations`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceServiceKeyOperations {
    pub default: Option<bool>,
    pub value: Option<Vec<OkmsKeyOps>>,
}

/// `okms.reference.serviceKey.ProtectionLevel`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceServiceKeyProtectionLevel {
    pub default: Option<bool>,
    pub value: Option<OkmsProtectionLevel>,
}

/// `okms.reference.serviceKey.Size`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceServiceKeySize {
    pub default: Option<bool>,
    pub value: Option<OkmsKeySize>,
}

/// `okms.reference.serviceKey.Response`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceServiceKeyResponse {
    pub curves: Option<Vec<ReferenceServiceKeyCurve>>,
    pub default: Option<bool>,
    pub operations: Option<Vec<ReferenceServiceKeyOperations>>,
    pub protection_level: Option<Vec<ReferenceServiceKeyProtectionLevel>>,
    pub sizes: Option<Vec<ReferenceServiceKeySize>>,
    #[serde(rename = "type")]
    pub kind: Option<OkmsKeyType>,
}

/// `okms.resource.Response`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceResponse {
    pub id: Option<String>,
    pub kmip_endpoint: Option<String>,
    pub kmip_object_count: Option<i64>,
    pub kmip_rsa_endpoint: Option<String>,
    pub pci_dss_enabled: Option<bool>,
    pub pci_dss_enabled_at: Option<String>,
    #[serde(default)]
    pub public_ca: serde_json::Value,
    #[serde(default)]
    pub public_rsa_ca: serde_json::Value,
    pub region: Option<OkmsRegion>,
    pub rest_endpoint: Option<String>,
    pub secret_count: Option<i64>,
    pub secret_version_count: Option<i64>,
    pub service_key_count: Option<i64>,
    pub swagger_endpoint: Option<String>,
}

/// `okms.resource.ResponseWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceResponseWithIAM {
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub id: Option<String>,
    pub kmip_endpoint: Option<String>,
    pub kmip_object_count: Option<i64>,
    pub kmip_rsa_endpoint: Option<String>,
    pub pci_dss_enabled: Option<bool>,
    pub pci_dss_enabled_at: Option<String>,
    #[serde(default)]
    pub public_ca: serde_json::Value,
    #[serde(default)]
    pub public_rsa_ca: serde_json::Value,
    pub region: Option<OkmsRegion>,
    pub rest_endpoint: Option<String>,
    pub secret_count: Option<i64>,
    pub secret_version_count: Option<i64>,
    pub service_key_count: Option<i64>,
    pub swagger_endpoint: Option<String>,
}

/// `okms.resource.UpdateRequest`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUpdateRequest {
    pub pci_dss_enabled: Option<bool>,
}

/// `okms.secret.MetadataRequest`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMetadataRequest {
    pub cas_required: Option<bool>,
    #[serde(default)]
    pub custom_metadata: serde_json::Value,
    pub deactivate_version_after: Option<String>,
    pub max_versions: Option<i64>,
}

/// `okms.secret.version.Creation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretVersionCreation {
    #[serde(default)]
    pub data: serde_json::Value,
}

/// `okms.secret.Creation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretCreation {
    pub metadata: Option<SecretMetadataRequest>,
    pub path: String,
    pub version: SecretVersionCreation,
}

/// `okms.secret.Metadata`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMetadata {
    pub cas_required: Option<bool>,
    pub created_at: Option<String>,
    pub current_version: Option<i64>,
    #[serde(default)]
    pub custom_metadata: serde_json::Value,
    pub deactivate_version_after: Option<String>,
    pub max_versions: Option<i64>,
    pub oldest_version: Option<i64>,
    pub updated_at: Option<String>,
}

/// `okms.secret.Version`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretVersion {
    pub created_at: Option<String>,
    #[serde(default)]
    pub data: serde_json::Value,
    pub deactivated_at: Option<String>,
    pub id: Option<i64>,
    pub state: Option<OkmsSecretState>,
}

/// `okms.secret.GetResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretGetResponse {
    pub metadata: Option<SecretMetadata>,
    pub path: Option<String>,
    pub version: Option<SecretVersion>,
}

/// `okms.secret.GetResponseWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretGetResponseWithIAM {
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub metadata: Option<SecretMetadata>,
    pub path: Option<String>,
    pub version: Option<SecretVersion>,
}

/// `okms.secret.PostResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretPostResponse {
    pub metadata: Option<SecretMetadata>,
    pub path: Option<String>,
}

/// `okms.secret.UpdateRequest`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretUpdateRequest {
    pub metadata: Option<SecretMetadataRequest>,
    pub version: Option<SecretVersionCreation>,
}

/// `okms.secret.version.UpdateRequest`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretVersionUpdateRequest {
    pub state: OkmsSecretState,
}

/// `okms.secret.version.UpdateResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretVersionUpdateResponse {
    pub created_at: Option<String>,
    pub deactivated_at: Option<String>,
    pub id: Option<i64>,
    pub state: Option<OkmsSecretState>,
}

/// `okms.secretConfig.Response`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretConfigResponse {
    pub cas_required: Option<bool>,
    pub deactivate_version_after: Option<String>,
    pub max_versions: Option<i64>,
}

/// `okms.secretConfig.UpdateRequest`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretConfigUpdateRequest {
    pub cas_required: Option<bool>,
    pub deactivate_version_after: Option<String>,
    pub max_versions: Option<i64>,
}

/// `okms.serviceKey.JsonWebKeyRequest`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceKeyJsonWebKeyRequest {
    pub alg: Option<OkmsKeyAlg>,
    pub crv: Option<OkmsKeyCurve>,
    #[serde(default)]
    pub d: serde_json::Value,
    #[serde(default)]
    pub dp: serde_json::Value,
    #[serde(default)]
    pub dq: serde_json::Value,
    pub e: Option<String>,
    #[serde(default)]
    pub k: serde_json::Value,
    pub key_ops: Option<Vec<OkmsKeyOps>>,
    pub kid: Option<String>,
    pub kty: OkmsKeyType,
    #[serde(default)]
    pub n: serde_json::Value,
    #[serde(default)]
    pub p: serde_json::Value,
    #[serde(default)]
    pub q: serde_json::Value,
    #[serde(default)]
    pub qi: serde_json::Value,
    pub use_: Option<OkmsKeyUse>,
    pub x: Option<String>,
    pub y: Option<String>,
}

/// `okms.serviceKey.Creation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceKeyCreation {
    pub context: Option<String>,
    pub curve: Option<OkmsKeyCurve>,
    pub keys: Option<Vec<ServiceKeyJsonWebKeyRequest>>,
    pub name: Option<String>,
    pub operations: Option<Vec<OkmsKeyOps>>,
    pub protection_level: Option<OkmsProtectionLevel>,
    pub size: Option<OkmsKeySize>,
    #[serde(rename = "type")]
    pub kind: Option<OkmsKeyType>,
}

/// `okms.serviceKey.JsonWebKeyResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceKeyJsonWebKeyResponse {
    pub alg: Option<OkmsKeyAlg>,
    pub crv: Option<OkmsKeyCurve>,
    #[serde(default)]
    pub d: serde_json::Value,
    #[serde(default)]
    pub dp: serde_json::Value,
    #[serde(default)]
    pub dq: serde_json::Value,
    pub e: Option<String>,
    #[serde(default)]
    pub k: serde_json::Value,
    pub key_ops: Option<Vec<OkmsKeyOps>>,
    pub kid: Option<String>,
    pub kty: Option<OkmsKeyType>,
    #[serde(default)]
    pub n: serde_json::Value,
    #[serde(default)]
    pub p: serde_json::Value,
    #[serde(default)]
    pub q: serde_json::Value,
    #[serde(default)]
    pub qi: serde_json::Value,
    pub use_: Option<OkmsKeyUse>,
    pub x: Option<String>,
    pub y: Option<String>,
}

/// `okms.serviceKey.PEM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceKeyPEM {
    pub pem: Option<String>,
}

/// `okms.serviceKey.Response`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceKeyResponse {
    pub created_at: Option<String>,
    pub curve: Option<OkmsKeyCurve>,
    pub id: Option<String>,
    pub keys: Option<Vec<ServiceKeyJsonWebKeyResponse>>,
    pub keys_pem: Option<Vec<ServiceKeyPEM>>,
    pub name: Option<String>,
    pub operations: Option<Vec<OkmsKeyOps>>,
    pub protection_level: Option<OkmsProtectionLevel>,
    pub size: Option<OkmsKeySize>,
    pub state: Option<OkmsKeyState>,
    #[serde(rename = "type")]
    pub kind: Option<OkmsKeyType>,
}

/// `okms.serviceKey.ResponseWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceKeyResponseWithIAM {
    pub created_at: Option<String>,
    pub curve: Option<OkmsKeyCurve>,
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub id: Option<String>,
    pub keys: Option<Vec<ServiceKeyJsonWebKeyResponse>>,
    pub keys_pem: Option<Vec<ServiceKeyPEM>>,
    pub name: Option<String>,
    pub operations: Option<Vec<OkmsKeyOps>>,
    pub protection_level: Option<OkmsProtectionLevel>,
    pub size: Option<OkmsKeySize>,
    pub state: Option<OkmsKeyState>,
    #[serde(rename = "type")]
    pub kind: Option<OkmsKeyType>,
}

/// `okms.serviceKey.Update`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceKeyUpdate {
    pub deactivation_reason: Option<OkmsKeyDeactivationReason>,
    pub name: Option<String>,
    pub state: Option<OkmsKeyStateUpdate>,
}

impl OvhClient {
    /// `GET /okms/reference/regions` — List available regions
    pub async fn okms_reference_regions(&self, page: &PageParams) -> Result<Vec<ReferenceRegion>> {
        self.get_page("/okms/reference/regions", &[], page)
            .await
            .map(|p| p.items)
    }

    /// `GET /okms/reference/secretConfig` — Get secret engine default configuration
    pub async fn okms_reference_secret_configs(
        &self,
        query: &[(&str, &str)],
    ) -> Result<ReferenceSecretConfigResponse> {
        self.get(&Self::append_query("/okms/reference/secretConfig", query))
            .await
    }

    /// `GET /okms/reference/serviceKey` — Get service key type, size, curve and operations combination
    pub async fn okms_reference_service_keys(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<ReferenceServiceKeyResponse>> {
        self.get_page(
            &Self::append_query("/okms/reference/serviceKey", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /okms/resource` — List OVHcloud KMS services
    pub async fn okms_resources(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<ResourceResponseWithIAM>> {
        self.get_page(&Self::append_query("/okms/resource", query), &[], page)
            .await
            .map(|p| p.items)
    }

    /// `GET /okms/resource/{okmsId}` — Get an OVHcloud KMS service
    pub async fn okms_resource(
        &self,
        okms_id: &str,
        query: &[(&str, &str)],
    ) -> Result<ResourceResponseWithIAM> {
        self.get(&Self::append_query(
            &format!("/okms/resource/{}", percent_encode(okms_id)),
            query,
        ))
        .await
    }

    /// `PUT /okms/resource/{okmsId}` — Update an OVHcloud KMS service
    pub async fn okms_resource_put(
        &self,
        okms_id: &str,
        body: &ResourceUpdateRequest,
    ) -> Result<ResourceResponse> {
        self.put_json(&format!("/okms/resource/{}", percent_encode(okms_id)), body)
            .await
    }

    /// `GET /okms/resource/{okmsId}/credential` — List all access credentials
    pub async fn okms_resource_credential(
        &self,
        okms_id: &str,
        page: &PageParams,
    ) -> Result<Vec<CredentialGetResponse>> {
        self.get_page(
            &format!("/okms/resource/{}/credential", percent_encode(okms_id)),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /okms/resource/{okmsId}/credential` — Request a new access credential
    pub async fn okms_resource_credential_post(
        &self,
        okms_id: &str,
        body: &CredentialCreation,
    ) -> Result<CredentialCreationResponse> {
        self.post_v2(
            &format!("/okms/resource/{}/credential", percent_encode(okms_id)),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /okms/resource/{okmsId}/credential/{credentialId}` — Revoke and delete an access credential
    pub async fn okms_resource_credential_delete(
        &self,
        credential_id: &str,
        okms_id: &str,
    ) -> Result<CredentialGetResponse> {
        self.delete_json(&format!(
            "/okms/resource/{}/credential/{}",
            percent_encode(okms_id),
            percent_encode(credential_id)
        ))
        .await
    }

    /// `GET /okms/resource/{okmsId}/credential/{credentialId}` — Get an access credential
    pub async fn okms_resource_credential_get(
        &self,
        credential_id: &str,
        okms_id: &str,
    ) -> Result<CredentialGetResponse> {
        self.get(&format!(
            "/okms/resource/{}/credential/{}",
            percent_encode(okms_id),
            percent_encode(credential_id)
        ))
        .await
    }

    /// `GET /okms/resource/{okmsId}/log/kind` — List available log kinds
    pub async fn okms_resource_log_kind(
        &self,
        okms_id: &str,
        page: &PageParams,
    ) -> Result<Vec<LogsLogKind>> {
        self.get_page(
            &format!("/okms/resource/{}/log/kind", percent_encode(okms_id)),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /okms/resource/{okmsId}/log/kind/{name}` — Get a log kind
    pub async fn okms_resource_log_kind_get(
        &self,
        name: &str,
        okms_id: &str,
    ) -> Result<LogsLogKind> {
        self.get(&format!(
            "/okms/resource/{}/log/kind/{}",
            percent_encode(okms_id),
            percent_encode(name)
        ))
        .await
    }

    /// `GET /okms/resource/{okmsId}/log/subscription` — List subscription IDs for a cluster
    pub async fn okms_resource_log_subscription(
        &self,
        okms_id: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<LogsLogSubscription>> {
        self.get_page(
            &Self::append_query(
                &format!(
                    "/okms/resource/{}/log/subscription",
                    percent_encode(okms_id)
                ),
                query,
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /okms/resource/{okmsId}/log/subscription` — Create a subscription from logs to a pre-existing LDP stream
    pub async fn okms_resource_log_subscription_post(
        &self,
        okms_id: &str,
        body: &LogsLogSubscriptionCreation,
    ) -> Result<LogsLogSubscriptionResponse> {
        self.post_v2(
            &format!(
                "/okms/resource/{}/log/subscription",
                percent_encode(okms_id)
            ),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /okms/resource/{okmsId}/log/subscription/{subscriptionId}` — Delete a subscription
    pub async fn okms_resource_log_subscription_delete(
        &self,
        okms_id: &str,
        subscription_id: &str,
    ) -> Result<LogsLogSubscriptionResponse> {
        self.delete_json(&format!(
            "/okms/resource/{}/log/subscription/{}",
            percent_encode(okms_id),
            percent_encode(subscription_id)
        ))
        .await
    }

    /// `GET /okms/resource/{okmsId}/log/subscription/{subscriptionId}` — Get subscription details
    pub async fn okms_resource_log_subscription_get(
        &self,
        okms_id: &str,
        subscription_id: &str,
    ) -> Result<LogsLogSubscription> {
        self.get(&format!(
            "/okms/resource/{}/log/subscription/{}",
            percent_encode(okms_id),
            percent_encode(subscription_id)
        ))
        .await
    }

    /// `POST /okms/resource/{okmsId}/log/url` — Generate a temporary URL to retrieve logs
    pub async fn okms_resource_log_url_post(
        &self,
        okms_id: &str,
        body: &LogsLogUrlCreation,
    ) -> Result<LogsTemporaryLogsLink> {
        self.post_v2(
            &format!("/okms/resource/{}/log/url", percent_encode(okms_id)),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `GET /okms/resource/{okmsId}/secret` — List all secrets
    pub async fn okms_resource_secret(
        &self,
        okms_id: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<SecretGetResponseWithIAM>> {
        self.get_page(
            &Self::append_query(
                &format!("/okms/resource/{}/secret", percent_encode(okms_id)),
                query,
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /okms/resource/{okmsId}/secret` — Create a secret
    pub async fn okms_resource_secret_post(
        &self,
        okms_id: &str,
        body: &SecretCreation,
    ) -> Result<SecretPostResponse> {
        self.post_v2(
            &format!("/okms/resource/{}/secret", percent_encode(okms_id)),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /okms/resource/{okmsId}/secret/{path}` — Delete a secret and all its versions
    pub async fn okms_resource_secret_delete(&self, okms_id: &str, path: &str) -> Result<()> {
        self.delete(&format!(
            "/okms/resource/{}/secret/{}",
            percent_encode(okms_id),
            percent_encode(path)
        ))
        .await
    }

    /// `GET /okms/resource/{okmsId}/secret/{path}` — Retrieve a secret
    pub async fn okms_resource_secret_get(
        &self,
        okms_id: &str,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<SecretGetResponseWithIAM> {
        self.get(&Self::append_query(
            &format!(
                "/okms/resource/{}/secret/{}",
                percent_encode(okms_id),
                percent_encode(path)
            ),
            query,
        ))
        .await
    }

    /// `PUT /okms/resource/{okmsId}/secret/{path}` — Update a secret
    pub async fn okms_resource_secret_put(
        &self,
        okms_id: &str,
        path: &str,
        query: &[(&str, &str)],
        body: &SecretUpdateRequest,
    ) -> Result<SecretPostResponse> {
        self.put_json(
            &format!(
                "/okms/resource/{}/secret/{}",
                percent_encode(okms_id),
                percent_encode(path)
            ),
            body,
        )
        .await
    }

    /// `GET /okms/resource/{okmsId}/secret/{path}/version` — List the versions of a secret
    pub async fn okms_resource_secret_version(
        &self,
        okms_id: &str,
        path: &str,
        page: &PageParams,
    ) -> Result<Vec<SecretVersion>> {
        self.get_page(
            &format!(
                "/okms/resource/{}/secret/{}/version",
                percent_encode(okms_id),
                percent_encode(path)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /okms/resource/{okmsId}/secret/{path}/version` — Create a secret version
    pub async fn okms_resource_secret_version_post(
        &self,
        okms_id: &str,
        path: &str,
        query: &[(&str, &str)],
        body: &SecretVersionCreation,
    ) -> Result<SecretVersion> {
        self.post_v2(
            &format!(
                "/okms/resource/{}/secret/{}/version",
                percent_encode(okms_id),
                percent_encode(path)
            ),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `GET /okms/resource/{okmsId}/secret/{path}/version/{version}` — Retrieve a secret version
    pub async fn okms_resource_secret_version_get(
        &self,
        okms_id: &str,
        path: &str,
        version: &str,
        query: &[(&str, &str)],
    ) -> Result<SecretVersion> {
        self.get(&Self::append_query(
            &format!(
                "/okms/resource/{}/secret/{}/version/{}",
                percent_encode(okms_id),
                percent_encode(path),
                percent_encode(version)
            ),
            query,
        ))
        .await
    }

    /// `PUT /okms/resource/{okmsId}/secret/{path}/version/{version}` — Update the state of a secret version
    pub async fn okms_resource_secret_version_put(
        &self,
        okms_id: &str,
        path: &str,
        version: &str,
        body: &SecretVersionUpdateRequest,
    ) -> Result<SecretVersionUpdateResponse> {
        self.put_json(
            &format!(
                "/okms/resource/{}/secret/{}/version/{}",
                percent_encode(okms_id),
                percent_encode(path),
                percent_encode(version)
            ),
            body,
        )
        .await
    }

    /// `GET /okms/resource/{okmsId}/secretConfig` — Retrieve secrets configuration
    pub async fn okms_resource_secret_config(&self, okms_id: &str) -> Result<SecretConfigResponse> {
        self.get(&format!(
            "/okms/resource/{}/secretConfig",
            percent_encode(okms_id)
        ))
        .await
    }

    /// `PUT /okms/resource/{okmsId}/secretConfig` — Update secrets configuration
    pub async fn okms_resource_secret_config_put(
        &self,
        okms_id: &str,
        body: &SecretConfigUpdateRequest,
    ) -> Result<SecretConfigResponse> {
        self.put_json(
            &format!("/okms/resource/{}/secretConfig", percent_encode(okms_id)),
            body,
        )
        .await
    }

    /// `GET /okms/resource/{okmsId}/serviceKey` — List all keys
    pub async fn okms_resource_service_key(
        &self,
        okms_id: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<ServiceKeyResponseWithIAM>> {
        self.get_page(
            &Self::append_query(
                &format!("/okms/resource/{}/serviceKey", percent_encode(okms_id)),
                query,
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /okms/resource/{okmsId}/serviceKey` — Create or import a service key
    pub async fn okms_resource_service_key_post(
        &self,
        okms_id: &str,
        body: &ServiceKeyCreation,
    ) -> Result<ServiceKeyResponse> {
        self.post_v2(
            &format!("/okms/resource/{}/serviceKey", percent_encode(okms_id)),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /okms/resource/{okmsId}/serviceKey/{keyId}` — Delete the given service key
    pub async fn okms_resource_service_key_delete(
        &self,
        key_id: &str,
        okms_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "/okms/resource/{}/serviceKey/{}",
            percent_encode(okms_id),
            percent_encode(key_id)
        ))
        .await
    }

    /// `GET /okms/resource/{okmsId}/serviceKey/{keyId}` — Retrieve a key
    pub async fn okms_resource_service_key_get(
        &self,
        key_id: &str,
        okms_id: &str,
        query: &[(&str, &str)],
    ) -> Result<ServiceKeyResponseWithIAM> {
        self.get(&Self::append_query(
            &format!(
                "/okms/resource/{}/serviceKey/{}",
                percent_encode(okms_id),
                percent_encode(key_id)
            ),
            query,
        ))
        .await
    }

    /// `PUT /okms/resource/{okmsId}/serviceKey/{keyId}` — Update a service key
    pub async fn okms_resource_service_key_put(
        &self,
        key_id: &str,
        okms_id: &str,
        body: &ServiceKeyUpdate,
    ) -> Result<ServiceKeyResponse> {
        self.put_json(
            &format!(
                "/okms/resource/{}/serviceKey/{}",
                percent_encode(okms_id),
                percent_encode(key_id)
            ),
            body,
        )
        .await
    }
}
