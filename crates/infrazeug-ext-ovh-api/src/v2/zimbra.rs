//! OVHcloud API v2 **zimbra** bindings (`/v2/zimbra`).
//!
//! Generated from the official schema; do not edit by hand.

#![allow(unused_imports, unused_variables)]

use crate::client::{percent_encode, OvhClient, PageParams, V2RequestOptions};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// `common.CurrentTaskStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommonCurrentTaskStatus {
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "PENDING")]
    Pending,
    #[serde(rename = "RUNNING")]
    Running,
    #[serde(rename = "SCHEDULED")]
    Scheduled,
    #[serde(rename = "WAITING_USER_INPUT")]
    WaitingUserInput,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `common.TaskError`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskError {
    pub message: Option<String>,
}

/// `common.CurrentTask`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentTask {
    pub errors: Option<Vec<TaskError>>,
    pub id: Option<String>,
    pub link: Option<String>,
    pub status: Option<CommonCurrentTaskStatus>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

/// `common.EventTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommonEventType {
    #[serde(rename = "TARGET_SPEC_UPDATE")]
    TargetSpecUpdate,
    #[serde(rename = "TASK_ERROR")]
    TaskError,
    #[serde(rename = "TASK_START")]
    TaskStart,
    #[serde(rename = "TASK_SUCCESS")]
    TaskSuccess,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `common.Event`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub created_at: Option<String>,
    pub kind: Option<String>,
    pub link: Option<String>,
    pub message: Option<String>,
    #[serde(rename = "type")]
    pub event_type: Option<CommonEventType>,
}

/// `common.ResourceStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommonResourceStatus {
    #[serde(rename = "CREATING")]
    Creating,
    #[serde(rename = "DELETING")]
    Deleting,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "OUT_OF_SYNC")]
    OutOfSync,
    #[serde(rename = "READY")]
    Ready,
    #[serde(rename = "SUSPENDED")]
    Suspended,
    #[serde(rename = "UNKNOWN")]
    UnknownValue,
    #[serde(rename = "UPDATING")]
    Updating,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `common.TaskStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommonTaskStatus {
    #[serde(rename = "DONE")]
    Done,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "PENDING")]
    Pending,
    #[serde(rename = "RUNNING")]
    Running,
    #[serde(rename = "SCHEDULED")]
    Scheduled,
    #[serde(rename = "WAITING_USER_INPUT")]
    WaitingUserInput,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `common.TaskProgress`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub name: Option<String>,
    pub status: Option<CommonTaskStatus>,
}

/// `common.Task`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub created_at: Option<String>,
    pub errors: Option<Vec<TaskError>>,
    pub finished_at: Option<String>,
    pub id: Option<String>,
    pub link: Option<String>,
    pub message: Option<String>,
    pub progress: Option<Vec<TaskProgress>>,
    pub started_at: Option<String>,
    pub status: Option<CommonTaskStatus>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub updated_at: Option<String>,
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

/// `zimbra.AccountStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZimbraAccountStatus {
    #[serde(rename = "BILLINGLOCKED")]
    Billinglocked,
    #[serde(rename = "BLOCKEDFORSPAM")]
    Blockedforspam,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `zimbra.AccountDetailedStatus`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDetailedStatus {
    pub details: Option<String>,
    pub link: Option<String>,
    pub status: Option<ZimbraAccountStatus>,
}

/// `zimbra.ContactInformation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactInformation {
    pub city: Option<String>,
    pub company: Option<String>,
    pub country: Option<String>,
    pub fax_number: Option<String>,
    pub mobile_number: Option<String>,
    pub office: Option<String>,
    pub phone_number: Option<String>,
    pub postcode: Option<String>,
    pub profession: Option<String>,
    pub service: Option<String>,
    pub street: Option<String>,
}

/// `zimbra.OfferEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZimbraOffer {
    #[serde(rename = "PRO")]
    Pro,
    #[serde(rename = "STARTER")]
    Starter,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `zimbra.Quota`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    pub available: Option<f64>,
    pub used: Option<f64>,
}

/// `zimbra.AccountCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountCurrentState {
    pub contact_information: Option<ContactInformation>,
    pub created_at: Option<String>,
    pub description: Option<String>,
    pub detailed_status: Option<Vec<AccountDetailedStatus>>,
    pub display_name: Option<String>,
    pub domain_id: Option<String>,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub hide_in_gal: Option<bool>,
    pub last_connection_at: Option<String>,
    pub last_name: Option<String>,
    pub offer: Option<ZimbraOffer>,
    pub organization_id: Option<String>,
    pub organization_label: Option<String>,
    pub quota: Option<Quota>,
    pub quota_refreshed_at: Option<String>,
    pub slot_id: Option<String>,
    pub updated_at: Option<String>,
}

/// `zimbra.AccountPostTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPostTargetSpec {
    pub contact_information: Option<ContactInformation>,
    pub description: Option<String>,
    pub display_name: Option<String>,
    pub email: String,
    pub first_name: Option<String>,
    pub force_change_password_after_login: Option<bool>,
    pub hide_in_gal: Option<bool>,
    pub last_name: Option<String>,
    pub offer: ZimbraOffer,
    #[serde(default)]
    pub password: serde_json::Value,
    pub quota: Option<Quota>,
    pub slot_id: Option<String>,
}

/// `zimbra.AccountPostPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPostPayload {
    pub target_spec: AccountPostTargetSpec,
}

/// `zimbra.AccountPutTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPutTargetSpec {
    pub contact_information: Option<ContactInformation>,
    pub description: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub force_change_password_after_login: Option<bool>,
    pub hide_in_gal: Option<bool>,
    pub last_name: Option<String>,
    pub password: Option<String>,
    pub quota: Option<Quota>,
    pub quota_refreshed_at: Option<String>,
}

/// `zimbra.AccountPutPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPutPayload {
    pub checksum: Option<String>,
    pub target_spec: AccountPutTargetSpec,
}

/// `zimbra.AccountTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTargetSpec {
    pub contact_information: Option<ContactInformation>,
    pub description: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub hide_in_gal: Option<bool>,
    pub last_name: Option<String>,
    pub quota_refreshed_at: Option<String>,
}

/// `zimbra.AccountResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResponse {
    pub checksum: Option<String>,
    pub current_state: Option<AccountCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<AccountTargetSpec>,
}

/// `zimbra.AliasSource`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasSource {
    pub domain_id: Option<String>,
    pub name: Option<String>,
    pub organization_id: Option<String>,
    pub organization_label: Option<String>,
}

/// `zimbra.AliasTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZimbraAliasType {
    #[serde(rename = "ACCOUNT")]
    Account,
    #[serde(rename = "MAILING_LIST")]
    MailingList,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `zimbra.AliasTarget`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasTarget {
    pub domain_id: Option<String>,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<ZimbraAliasType>,
}

/// `zimbra.Alias`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alias {
    pub alias: Option<AliasSource>,
    pub target: Option<AliasTarget>,
}

/// `zimbra.AliasBase`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasBase {
    pub alias: String,
    pub target_id: String,
}

/// `zimbra.AliasPostPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasPostPayload {
    pub target_spec: AliasBase,
}

/// `zimbra.AliasResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasResponse {
    pub checksum: Option<String>,
    pub current_state: Option<Alias>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<AliasBase>,
}

/// `zimbra.BillingStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZimbraBillingStatus {
    #[serde(rename = "CREATING")]
    Creating,
    #[serde(rename = "DELETED")]
    Deleted,
    #[serde(rename = "DELETING")]
    Deleting,
    #[serde(rename = "OK")]
    Ok,
    #[serde(rename = "REOPENING")]
    Reopening,
    #[serde(rename = "SUSPENDED")]
    Suspended,
    #[serde(rename = "SUSPENDING")]
    Suspending,
    #[serde(rename = "UPDATING")]
    Updating,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `zimbra.CName`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CName {
    pub name: Option<String>,
    pub value: Option<String>,
}

/// `zimbra.DKIMSelectors`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DKIMSelectors {
    pub cnames: Option<Vec<CName>>,
}

/// `zimbra.DNSOwnership`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DNSOwnership {
    pub cname: Option<String>,
}

/// `zimbra.DomainStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZimbraDomainStatus {
    #[serde(rename = "READY")]
    Ready,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `zimbra.MXRecord`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MXRecord {
    pub priority: Option<i64>,
    pub target: Option<String>,
}

/// `zimbra.ExpectedDNSConfig`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedDNSConfig {
    pub autodiscover: Option<String>,
    pub dkim: Option<DKIMSelectors>,
    pub mx: Option<Vec<MXRecord>>,
    pub ownership: Option<DNSOwnership>,
    pub spf: Option<String>,
}

/// `zimbra.OfferStatistics`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferStatistics {
    pub available_accounts_count: Option<i64>,
    pub configured_accounts_count: Option<i64>,
    pub offer: Option<ZimbraOffer>,
}

/// `zimbra.DomainCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainCurrentState {
    pub accounts_statistics: Option<Vec<OfferStatistics>>,
    pub created_at: Option<String>,
    pub expected_dnsconfig: Option<ExpectedDNSConfig>,
    pub name: Option<String>,
    pub organization_id: Option<String>,
    pub organization_label: Option<String>,
    pub status: Option<ZimbraDomainStatus>,
    pub updated_at: Option<String>,
}

/// `zimbra.DomainDiagnosisErrorCodeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZimbraDomainDiagnosisErrorCode {
    #[serde(rename = "BAD_CONFIGURATION")]
    BadConfiguration,
    #[serde(rename = "DOMAIN_IN_TRANSIENT_STATE")]
    DomainInTransientState,
    #[serde(rename = "DOMAIN_NOT_FOUND")]
    DomainNotFound,
    #[serde(rename = "DOMAIN_NOT_VALIDATED")]
    DomainNotValidated,
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `zimbra.DomainDiagnosisError`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisError {
    pub code: Option<ZimbraDomainDiagnosisErrorCode>,
    pub message: Option<String>,
}

/// `zimbra.DomainDiagnosisPostPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisPostPayload {
    #[serde(default)]
    pub domains: Vec<String>,
}

/// `zimbra.DomainDiagnosisRecommendations`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisRecommendations {
    pub expected_dnsconfig: Option<ExpectedDNSConfig>,
}

/// `zimbra.DomainDiagnosisTestAutodiscoverErrorCodeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZimbraDomainDiagnosisTestAutodiscoverErrorCode {
    #[serde(rename = "INCORRECT_SRV_RECORD")]
    IncorrectSrvRecord,
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,
    #[serde(rename = "MULTIPLE_SRV_RECORDS")]
    MultipleSrvRecords,
    #[serde(rename = "NO_SRV_RECORD")]
    NoSrvRecord,
    #[serde(rename = "TASK_FAILED")]
    TaskFailed,
    #[serde(rename = "TASK_RUNNING")]
    TaskRunning,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `zimbra.DomainDiagnosisTestAutodiscoverError`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisTestAutodiscoverError {
    pub code: Option<ZimbraDomainDiagnosisTestAutodiscoverErrorCode>,
    pub message: Option<String>,
}

/// `zimbra.DomainDiagnosisTestStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZimbraDomainDiagnosisTestStatus {
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "OK")]
    Ok,
    #[serde(rename = "WARNING")]
    Warning,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `zimbra.DomainDiagnosisTestAutodiscoverResult`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisTestAutodiscoverResult {
    pub errors: Option<Vec<DomainDiagnosisTestAutodiscoverError>>,
    pub records_found: Option<Vec<String>>,
    pub status: Option<ZimbraDomainDiagnosisTestStatus>,
}

/// `zimbra.DomainDiagnosisTestDKIMErrorCodeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZimbraDomainDiagnosisTestDKIMErrorCode {
    #[serde(rename = "DKIM_DISABLED")]
    DkimDisabled,
    #[serde(rename = "INCORRECT_CNAME_RECORD")]
    IncorrectCnameRecord,
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,
    #[serde(rename = "MISSING_ONE_SELECTOR")]
    MissingOneSelector,
    #[serde(rename = "OVH_NOT_INCLUDED")]
    OvhNotIncluded,
    #[serde(rename = "TASK_FAILED")]
    TaskFailed,
    #[serde(rename = "TASK_RUNNING")]
    TaskRunning,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `zimbra.DomainDiagnosisTestDKIMError`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisTestDKIMError {
    pub code: Option<ZimbraDomainDiagnosisTestDKIMErrorCode>,
    pub message: Option<String>,
}

/// `zimbra.DomainDiagnosisTestDKIMResult`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisTestDKIMResult {
    pub errors: Option<Vec<DomainDiagnosisTestDKIMError>>,
    pub records_found: Option<Vec<CName>>,
    pub status: Option<ZimbraDomainDiagnosisTestStatus>,
}

/// `zimbra.DomainDiagnosisTestMXErrorCodeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZimbraDomainDiagnosisTestMXErrorCode {
    #[serde(rename = "EXTERNAL_MX_RECORD")]
    ExternalMxRecord,
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,
    #[serde(rename = "MISSING_OVH_SERVER")]
    MissingOvhServer,
    #[serde(rename = "NO_MX_RECORD")]
    NoMxRecord,
    #[serde(rename = "OVH_MX_LOW_PRIORITY")]
    OvhMxLowPriority,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `zimbra.DomainDiagnosisTestMXError`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisTestMXError {
    pub code: Option<ZimbraDomainDiagnosisTestMXErrorCode>,
    pub message: Option<String>,
}

/// `zimbra.DomainDiagnosisTestMXResult`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisTestMXResult {
    pub errors: Option<Vec<DomainDiagnosisTestMXError>>,
    pub records_found: Option<Vec<MXRecord>>,
    pub status: Option<ZimbraDomainDiagnosisTestStatus>,
}

/// `zimbra.DomainDiagnosisTestSPFErrorCodeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZimbraDomainDiagnosisTestSPFErrorCode {
    #[serde(rename = "DANGEROUS_SPF_POLICY")]
    DangerousSpfPolicy,
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,
    #[serde(rename = "INVALID_SPF_RECORD")]
    InvalidSpfRecord,
    #[serde(rename = "MISSING_OVH_SERVER")]
    MissingOvhServer,
    #[serde(rename = "MISSING_SPF_POLICY")]
    MissingSpfPolicy,
    #[serde(rename = "MULTIPLE_SPF_RECORDS")]
    MultipleSpfRecords,
    #[serde(rename = "NOT_RECOMMENDED_SPF_POLICY")]
    NotRecommendedSpfPolicy,
    #[serde(rename = "NO_SPF_RECORD")]
    NoSpfRecord,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `zimbra.DomainDiagnosisTestSPFError`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisTestSPFError {
    pub code: Option<ZimbraDomainDiagnosisTestSPFErrorCode>,
    pub message: Option<String>,
}

/// `zimbra.DomainDiagnosisTestSPFResult`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisTestSPFResult {
    pub errors: Option<Vec<DomainDiagnosisTestSPFError>>,
    pub records_found: Option<Vec<String>>,
    pub status: Option<ZimbraDomainDiagnosisTestStatus>,
}

/// `zimbra.DomainDiagnosisResult`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisResult {
    pub autodiscover: Option<DomainDiagnosisTestAutodiscoverResult>,
    pub dkim: Option<DomainDiagnosisTestDKIMResult>,
    pub mx: Option<DomainDiagnosisTestMXResult>,
    pub spf: Option<DomainDiagnosisTestSPFResult>,
}

/// `zimbra.DomainDiagnosisStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ZimbraDomainDiagnosisStatus {
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "OK")]
    Ok,
    #[serde(rename = "PARTIAL")]
    Partial,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `zimbra.DomainDiagnosisResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainDiagnosisResponse {
    pub domain_id: Option<String>,
    pub domain_name: Option<String>,
    pub error: Option<DomainDiagnosisError>,
    pub is_external: Option<bool>,
    pub recommendations: Option<DomainDiagnosisRecommendations>,
    pub result: Option<DomainDiagnosisResult>,
    pub status: Option<ZimbraDomainDiagnosisStatus>,
}

/// `zimbra.DomainPostTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainPostTargetSpec {
    pub auto_configure_autodiscover: Option<bool>,
    pub auto_configure_dkim: bool,
    pub auto_configure_mx: bool,
    pub auto_configure_spf: bool,
    pub name: String,
    pub organization_id: String,
}

/// `zimbra.DomainPostPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainPostPayload {
    pub target_spec: DomainPostTargetSpec,
}

/// `zimbra.DomainPutTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainPutTargetSpec {
    pub dkim_enabled: Option<bool>,
    pub organization_id: Option<String>,
}

/// `zimbra.DomainPutPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainPutPayload {
    pub checksum: Option<String>,
    pub target_spec: DomainPutTargetSpec,
}

/// `zimbra.DomainResponseTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainResponseTargetSpec {
    pub dkim_enabled: Option<bool>,
    pub organization_id: Option<String>,
}

/// `zimbra.DomainResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainResponse {
    pub checksum: Option<String>,
    pub current_state: Option<DomainCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<DomainResponseTargetSpec>,
}

/// `zimbra.MailingList`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailingList {
    pub default_reply_to: Option<String>,
    pub email: Option<String>,
    pub language: Option<String>,
    pub members: Option<Vec<String>>,
    pub moderation_option: Option<String>,
    pub organization_id: Option<String>,
    pub organization_label: Option<String>,
    pub owner: Option<String>,
}

/// `zimbra.MailingListBase`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailingListBase {
    pub default_reply_to: Option<String>,
    pub email: Option<String>,
    pub language: Option<String>,
    pub members: Option<Vec<String>>,
    pub moderation_option: Option<String>,
    pub organization_id: Option<String>,
    pub owner: Option<String>,
}

/// `zimbra.MailingListPostPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailingListPostPayload {
    pub target_spec: MailingListBase,
}

/// `zimbra.MailingListPutPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailingListPutPayload {
    pub checksum: Option<String>,
    pub target_spec: Option<MailingListBase>,
}

/// `zimbra.MailingListResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailingListResponse {
    pub checksum: Option<String>,
    pub current_state: Option<MailingList>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<MailingList>,
}

/// `zimbra.OrganizationCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationCurrentState {
    pub accounts_statistics: Option<Vec<OfferStatistics>>,
    pub created_at: Option<String>,
    pub description: Option<String>,
    pub label: Option<String>,
    pub name: Option<String>,
    pub storage_consumed: Option<f64>,
    pub updated_at: Option<String>,
}

/// `zimbra.OrganizationPostTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPostTargetSpec {
    pub description: Option<String>,
    pub label: String,
    pub name: String,
}

/// `zimbra.OrganizationPostPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPostPayload {
    pub target_spec: OrganizationPostTargetSpec,
}

/// `zimbra.OrganizationPutTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPutTargetSpec {
    pub description: Option<String>,
    pub label: Option<String>,
    pub name: Option<String>,
}

/// `zimbra.OrganizationPutPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPutPayload {
    pub checksum: Option<String>,
    pub target_spec: OrganizationPutTargetSpec,
}

/// `zimbra.OrganizationResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationResponse {
    pub checksum: Option<String>,
    pub current_state: Option<OrganizationCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<OrganizationPutTargetSpec>,
}

/// `zimbra.PlatformCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCurrentState {
    pub accounts_statistics: Option<Vec<OfferStatistics>>,
    pub description: Option<String>,
    pub name: Option<String>,
    pub number_of_organizations: Option<i64>,
    pub quota: Option<f64>,
}

/// `zimbra.PlatformPostTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformPostTargetSpec {
    pub description: String,
    pub name: String,
}

/// `zimbra.PlatformPostPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformPostPayload {
    pub target_spec: PlatformPostTargetSpec,
}

/// `zimbra.PlatformPutTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformPutTargetSpec {
    pub description: Option<String>,
    pub name: Option<String>,
}

/// `zimbra.PlatformPutPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformPutPayload {
    pub checksum: Option<String>,
    pub target_spec: PlatformPutTargetSpec,
}

/// `zimbra.PlatformResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformResponse {
    pub checksum: Option<String>,
    pub current_state: Option<PlatformCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<PlatformPutTargetSpec>,
}

/// `zimbra.PlatformResponseWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformResponseWithIAM {
    pub checksum: Option<String>,
    pub current_state: Option<PlatformCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<PlatformPutTargetSpec>,
}

/// `zimbra.Project`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub description: Option<String>,
    pub name: Option<String>,
    pub number_of_platforms: Option<i64>,
    pub total_storage: Option<i64>,
}

/// `zimbra.ProjectResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResponse {
    pub checksum: Option<String>,
    pub current_state: Option<Project>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<Project>,
}

/// `zimbra.ProjectResponseWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResponseWithIAM {
    pub checksum: Option<String>,
    pub current_state: Option<Project>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<Project>,
}

/// `zimbra.Redirection`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Redirection {
    pub created_at: Option<String>,
    pub destination: Option<String>,
    pub domain_id: Option<String>,
    pub organization_id: Option<String>,
    pub organization_label: Option<String>,
    pub source: Option<String>,
    pub updated_at: Option<String>,
}

/// `zimbra.RedirectionBase`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectionBase {
    pub destination: String,
    pub source: String,
}

/// `zimbra.RedirectionPostPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectionPostPayload {
    pub target_spec: RedirectionBase,
}

/// `zimbra.RedirectionResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectionResponse {
    pub checksum: Option<String>,
    pub current_state: Option<Redirection>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<RedirectionBase>,
}

/// `zimbra.SlotCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotCurrentState {
    pub account_id: Option<String>,
    pub billing_status: Option<ZimbraBillingStatus>,
    pub created_at: Option<String>,
    pub domain_promotion_link: Option<String>,
    pub email: Option<String>,
    pub offer: Option<ZimbraOffer>,
    pub platform_id: Option<String>,
}

/// `zimbra.SlotResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotResponse {
    pub checksum: Option<String>,
    pub current_state: Option<SlotCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
}

impl OvhClient {
    /// `GET /zimbra/platform` — Get a list of Zimbra Platforms
    pub async fn zimbra_platforms(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<PlatformResponseWithIAM>> {
        self.get_page(&Self::append_query("/zimbra/platform", query), &[], page)
            .await
            .map(|p| p.items)
    }

    /// `GET /zimbra/platform/{platformId}` — Get a Zimbra Platform
    pub async fn zimbra_platform(&self, platform_id: &str) -> Result<PlatformResponseWithIAM> {
        self.get(&format!("/zimbra/platform/{}", percent_encode(platform_id)))
            .await
    }

    /// `PUT /zimbra/platform/{platformId}` — Modify a platform
    pub async fn zimbra_platform_put(
        &self,
        platform_id: &str,
        body: &PlatformPutPayload,
    ) -> Result<PlatformResponse> {
        self.put_json(
            &format!("/zimbra/platform/{}", percent_encode(platform_id)),
            body,
        )
        .await
    }

    /// `GET /zimbra/platform/{platformId}/account` — Get list of accounts
    pub async fn zimbra_platform_account(
        &self,
        platform_id: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<AccountResponse>> {
        self.get_page(
            &Self::append_query(
                &format!("/zimbra/platform/{}/account", percent_encode(platform_id)),
                query,
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /zimbra/platform/{platformId}/account` — Create an account
    pub async fn zimbra_platform_account_post(
        &self,
        platform_id: &str,
        body: &AccountPostPayload,
    ) -> Result<AccountResponse> {
        self.post_v2(
            &format!("/zimbra/platform/{}/account", percent_encode(platform_id)),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /zimbra/platform/{platformId}/account/{accountId}` — Delete an account
    pub async fn zimbra_platform_account_delete(
        &self,
        account_id: &str,
        platform_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "/zimbra/platform/{}/account/{}",
            percent_encode(platform_id),
            percent_encode(account_id)
        ))
        .await
    }

    /// `GET /zimbra/platform/{platformId}/account/{accountId}` — Get an account
    pub async fn zimbra_platform_account_get(
        &self,
        account_id: &str,
        platform_id: &str,
    ) -> Result<AccountResponse> {
        self.get(&format!(
            "/zimbra/platform/{}/account/{}",
            percent_encode(platform_id),
            percent_encode(account_id)
        ))
        .await
    }

    /// `PUT /zimbra/platform/{platformId}/account/{accountId}` — Modify an account
    pub async fn zimbra_platform_account_put(
        &self,
        account_id: &str,
        platform_id: &str,
        body: &AccountPutPayload,
    ) -> Result<AccountResponse> {
        self.put_json(
            &format!(
                "/zimbra/platform/{}/account/{}",
                percent_encode(platform_id),
                percent_encode(account_id)
            ),
            body,
        )
        .await
    }

    /// `GET /zimbra/platform/{platformId}/alias` — Retrieve the list of platform aliases
    pub async fn zimbra_platform_alias(
        &self,
        platform_id: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<AliasResponse>> {
        self.get_page(
            &Self::append_query(
                &format!("/zimbra/platform/{}/alias", percent_encode(platform_id)),
                query,
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /zimbra/platform/{platformId}/alias` — Create an alias
    pub async fn zimbra_platform_alias_post(
        &self,
        platform_id: &str,
        body: &AliasPostPayload,
    ) -> Result<AliasResponse> {
        self.post_v2(
            &format!("/zimbra/platform/{}/alias", percent_encode(platform_id)),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /zimbra/platform/{platformId}/alias/{aliasId}` — Delete an alias
    pub async fn zimbra_platform_alias_delete(
        &self,
        alias_id: &str,
        platform_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "/zimbra/platform/{}/alias/{}",
            percent_encode(platform_id),
            percent_encode(alias_id)
        ))
        .await
    }

    /// `GET /zimbra/platform/{platformId}/alias/{aliasId}` — Retrieve a platform alias
    pub async fn zimbra_platform_alias_get(
        &self,
        alias_id: &str,
        platform_id: &str,
    ) -> Result<AliasResponse> {
        self.get(&format!(
            "/zimbra/platform/{}/alias/{}",
            percent_encode(platform_id),
            percent_encode(alias_id)
        ))
        .await
    }

    /// `POST /zimbra/platform/{platformId}/diagnostic/domain` —
    pub async fn zimbra_platform_diagnostic_domain_post(
        &self,
        platform_id: &str,
        body: &DomainDiagnosisPostPayload,
    ) -> Result<Vec<DomainDiagnosisResponse>> {
        self.post_v2(
            &format!(
                "/zimbra/platform/{}/diagnostic/domain",
                percent_encode(platform_id)
            ),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `GET /zimbra/platform/{platformId}/domain` — Get list of domains
    pub async fn zimbra_platform_domain(
        &self,
        platform_id: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<DomainResponse>> {
        self.get_page(
            &Self::append_query(
                &format!("/zimbra/platform/{}/domain", percent_encode(platform_id)),
                query,
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /zimbra/platform/{platformId}/domain` — Create a domain
    pub async fn zimbra_platform_domain_post(
        &self,
        platform_id: &str,
        body: &DomainPostPayload,
    ) -> Result<DomainResponse> {
        self.post_v2(
            &format!("/zimbra/platform/{}/domain", percent_encode(platform_id)),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /zimbra/platform/{platformId}/domain/{domainId}` — Delete a domain
    pub async fn zimbra_platform_domain_delete(
        &self,
        domain_id: &str,
        platform_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "/zimbra/platform/{}/domain/{}",
            percent_encode(platform_id),
            percent_encode(domain_id)
        ))
        .await
    }

    /// `GET /zimbra/platform/{platformId}/domain/{domainId}` — Get a domain
    pub async fn zimbra_platform_domain_get(
        &self,
        domain_id: &str,
        platform_id: &str,
    ) -> Result<DomainResponse> {
        self.get(&format!(
            "/zimbra/platform/{}/domain/{}",
            percent_encode(platform_id),
            percent_encode(domain_id)
        ))
        .await
    }

    /// `PUT /zimbra/platform/{platformId}/domain/{domainId}` — Modify a domain
    pub async fn zimbra_platform_domain_put(
        &self,
        domain_id: &str,
        platform_id: &str,
        body: &DomainPutPayload,
    ) -> Result<DomainResponse> {
        self.put_json(
            &format!(
                "/zimbra/platform/{}/domain/{}",
                percent_encode(platform_id),
                percent_encode(domain_id)
            ),
            body,
        )
        .await
    }

    /// `GET /zimbra/platform/{platformId}/organization` — Get list of organizations
    pub async fn zimbra_platform_organization(
        &self,
        platform_id: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<OrganizationResponse>> {
        self.get_page(
            &Self::append_query(
                &format!(
                    "/zimbra/platform/{}/organization",
                    percent_encode(platform_id)
                ),
                query,
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /zimbra/platform/{platformId}/organization` — Create an organization
    pub async fn zimbra_platform_organization_post(
        &self,
        platform_id: &str,
        body: &OrganizationPostPayload,
    ) -> Result<OrganizationResponse> {
        self.post_v2(
            &format!(
                "/zimbra/platform/{}/organization",
                percent_encode(platform_id)
            ),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /zimbra/platform/{platformId}/organization/{organizationId}` — Delete an organization
    pub async fn zimbra_platform_organization_delete(
        &self,
        organization_id: &str,
        platform_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "/zimbra/platform/{}/organization/{}",
            percent_encode(platform_id),
            percent_encode(organization_id)
        ))
        .await
    }

    /// `GET /zimbra/platform/{platformId}/organization/{organizationId}` — Get an organization
    pub async fn zimbra_platform_organization_get(
        &self,
        organization_id: &str,
        platform_id: &str,
    ) -> Result<OrganizationResponse> {
        self.get(&format!(
            "/zimbra/platform/{}/organization/{}",
            percent_encode(platform_id),
            percent_encode(organization_id)
        ))
        .await
    }

    /// `PUT /zimbra/platform/{platformId}/organization/{organizationId}` — Modify an organization
    pub async fn zimbra_platform_organization_put(
        &self,
        organization_id: &str,
        platform_id: &str,
        body: &OrganizationPutPayload,
    ) -> Result<OrganizationResponse> {
        self.put_json(
            &format!(
                "/zimbra/platform/{}/organization/{}",
                percent_encode(platform_id),
                percent_encode(organization_id)
            ),
            body,
        )
        .await
    }

    /// `GET /zimbra/platform/{platformId}/redirection` — Get a platform redirection list
    pub async fn zimbra_platform_redirection(
        &self,
        platform_id: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<RedirectionResponse>> {
        self.get_page(
            &Self::append_query(
                &format!(
                    "/zimbra/platform/{}/redirection",
                    percent_encode(platform_id)
                ),
                query,
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /zimbra/platform/{platformId}/redirection` — Create an redirection
    pub async fn zimbra_platform_redirection_post(
        &self,
        platform_id: &str,
        body: &RedirectionPostPayload,
    ) -> Result<RedirectionResponse> {
        self.post_v2(
            &format!(
                "/zimbra/platform/{}/redirection",
                percent_encode(platform_id)
            ),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /zimbra/platform/{platformId}/redirection/{redirectionId}` — Delete an redirection
    pub async fn zimbra_platform_redirection_delete(
        &self,
        platform_id: &str,
        redirection_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "/zimbra/platform/{}/redirection/{}",
            percent_encode(platform_id),
            percent_encode(redirection_id)
        ))
        .await
    }

    /// `GET /zimbra/platform/{platformId}/redirection/{redirectionId}` — Get a platform redirection
    pub async fn zimbra_platform_redirection_get(
        &self,
        platform_id: &str,
        redirection_id: &str,
    ) -> Result<RedirectionResponse> {
        self.get(&format!(
            "/zimbra/platform/{}/redirection/{}",
            percent_encode(platform_id),
            percent_encode(redirection_id)
        ))
        .await
    }

    /// `GET /zimbra/platform/{platformId}/slot` — Get a platform slot list
    pub async fn zimbra_platform_slot(
        &self,
        platform_id: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<SlotResponse>> {
        self.get_page(
            &Self::append_query(
                &format!("/zimbra/platform/{}/slot", percent_encode(platform_id)),
                query,
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /zimbra/platform/{platformId}/slot/{slotId}` — Get a platform slot
    pub async fn zimbra_platform_slot_get(
        &self,
        platform_id: &str,
        slot_id: &str,
    ) -> Result<SlotResponse> {
        self.get(&format!(
            "/zimbra/platform/{}/slot/{}",
            percent_encode(platform_id),
            percent_encode(slot_id)
        ))
        .await
    }

    /// `GET /zimbra/platform/{platformId}/task` — Get a list of platform tasks
    pub async fn zimbra_platform_task(
        &self,
        platform_id: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<Task>> {
        self.get_page(
            &Self::append_query(
                &format!("/zimbra/platform/{}/task", percent_encode(platform_id)),
                query,
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }
}
