//! OVHcloud API v2 **managedCMS** bindings (`/v2/managedCMS`).
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

/// `common.TaskWithInputs<T>`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskWithInputsT {
    pub created_at: Option<String>,
    pub errors: Option<Vec<TaskError>>,
    pub finished_at: Option<String>,
    pub id: Option<String>,
    #[serde(default)]
    pub inputs: serde_json::Value,
    pub link: Option<String>,
    pub message: Option<String>,
    pub progress: Option<Vec<TaskProgress>>,
    pub started_at: Option<String>,
    pub status: Option<CommonTaskStatus>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub updated_at: Option<String>,
}

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

/// `managedCMS.CMSEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManagedCMSCMS {
    #[serde(rename = "WORDPRESS")]
    Wordpress,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `managedCMS.CMSSpecific.WordPress.LanguageEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManagedCMSCMSSpecificWordPressLanguage {
    #[serde(rename = "de_DE")]
    DeDe,
    #[serde(rename = "en_GB")]
    EnGb,
    #[serde(rename = "en_US")]
    EnUs,
    #[serde(rename = "es_ES")]
    EsEs,
    #[serde(rename = "fr_CA")]
    FrCa,
    #[serde(rename = "fr_FR")]
    FrFr,
    #[serde(rename = "it_IT")]
    ItIt,
    #[serde(rename = "pl_PL")]
    PlPl,
    #[serde(rename = "pt_PT")]
    PtPt,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `managedCMS.CMSSpecific.WordPress.WebsiteCreationTargetSpecCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CMSSpecificWordPressWebsiteCreationTargetSpecCreation {
    pub language: Option<ManagedCMSCMSSpecificWordPressLanguage>,
}

/// `managedCMS.CMSSpecific.WebsiteCreationTargetSpecCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CMSSpecificWebsiteCreationTargetSpecCreation {
    pub wordpress: Option<CMSSpecificWordPressWebsiteCreationTargetSpecCreation>,
}

/// `managedCMS.CMSSpecific.WordPress.WebsiteImportPlugin`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CMSSpecificWordPressWebsiteImportPlugin {
    pub enabled: Option<bool>,
    pub name: Option<String>,
    pub version: Option<String>,
}

/// `managedCMS.CMSSpecific.WordPress.WebsiteImportTheme`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CMSSpecificWordPressWebsiteImportTheme {
    pub active: Option<bool>,
    pub name: Option<String>,
    pub version: Option<String>,
}

/// `managedCMS.CMSSpecific.WordPress.WebsiteImportCheckResult`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CMSSpecificWordPressWebsiteImportCheckResult {
    pub plugins: Option<Vec<CMSSpecificWordPressWebsiteImportPlugin>>,
    pub themes: Option<Vec<CMSSpecificWordPressWebsiteImportTheme>>,
}

/// `managedCMS.CMSSpecific.WebsiteImportCheckResult`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CMSSpecificWebsiteImportCheckResult {
    pub wordpress: Option<CMSSpecificWordPressWebsiteImportCheckResult>,
}

/// `managedCMS.CMSSpecific.WordPress.WebsiteTargetSpecCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CMSSpecificWordPressWebsiteTargetSpecCreation {
    pub language: Option<ManagedCMSCMSSpecificWordPressLanguage>,
}

/// `managedCMS.CMSSpecific.WebsiteTargetSpecCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CMSSpecificWebsiteTargetSpecCreation {
    pub wordpress: Option<CMSSpecificWordPressWebsiteTargetSpecCreation>,
}

/// `managedCMS.CMSSpecific.WordPress.WebsiteImportSelection`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CMSSpecificWordPressWebsiteImportSelection {
    pub comments: Option<bool>,
    pub media: Option<bool>,
    pub pages: Option<bool>,
    pub plugins: Option<Vec<CMSSpecificWordPressWebsiteImportPlugin>>,
    pub posts: Option<bool>,
    pub tags: Option<bool>,
    pub themes: Option<Vec<CMSSpecificWordPressWebsiteImportTheme>>,
    pub users: Option<bool>,
    pub whole_database: Option<bool>,
}

/// `managedCMS.CMSSpecific.WordPress.WebsiteTargetSpecImport`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CMSSpecificWordPressWebsiteTargetSpecImport {
    pub selection: Option<CMSSpecificWordPressWebsiteImportSelection>,
}

/// `managedCMS.CMSSpecific.WebsiteTargetSpecImport`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CMSSpecificWebsiteTargetSpecImport {
    pub wordpress: Option<CMSSpecificWordPressWebsiteTargetSpecImport>,
}

/// `managedCMS.Domain`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Domain {
    pub fqdn: Option<String>,
}

/// `managedCMS.LanguageEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManagedCMSLanguage {
    #[serde(rename = "de_DE")]
    DeDe,
    #[serde(rename = "en_GB")]
    EnGb,
    #[serde(rename = "en_US")]
    EnUs,
    #[serde(rename = "es_ES")]
    EsEs,
    #[serde(rename = "fr_CA")]
    FrCa,
    #[serde(rename = "fr_FR")]
    FrFr,
    #[serde(rename = "it_IT")]
    ItIt,
    #[serde(rename = "pl_PL")]
    PlPl,
    #[serde(rename = "pt_PT")]
    PtPt,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `managedCMS.Language`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Language {
    pub code: Option<ManagedCMSLanguage>,
    pub name: Option<String>,
}

/// `managedCMS.PHPVersionEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManagedCMSPHPVersion {
    #[serde(rename = "7.4")]
    V74,
    #[serde(rename = "8.1")]
    V81,
    #[serde(rename = "8.2")]
    V82,
    #[serde(rename = "8.3")]
    V83,
    #[serde(rename = "8.4")]
    V84,
    #[serde(rename = "8.5")]
    V85,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `managedCMS.ServiceDashboards`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDashboards {
    pub wordpress: Option<String>,
}

/// `managedCMS.ServiceDiskQuota`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDiskQuota {
    pub additional_quota_bytes: Option<i64>,
    pub plan_quota_bytes: Option<i64>,
    pub total_quota_bytes: Option<i64>,
    pub total_usage_bytes: Option<i64>,
}

/// `managedCMS.VisitsBoost`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisitsBoost {
    pub created_at: Option<String>,
    pub current_amount: Option<i64>,
    pub expires_at: Option<String>,
    pub initial_amount: Option<i64>,
}

/// `managedCMS.ServiceVisitsQuota`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceVisitsQuota {
    pub boosts: Option<Vec<VisitsBoost>>,
    pub plan_quota: Option<i64>,
    pub total_additional_quota: Option<i64>,
    pub total_quota: Option<i64>,
    pub total_usage: Option<i64>,
}

/// `managedCMS.ServiceWebsitesQuota`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceWebsitesQuota {
    pub additional_quota: Option<i64>,
    pub plan_quota: Option<i64>,
    pub total_quota: Option<i64>,
    pub total_usage: Option<i64>,
}

/// `managedCMS.ServiceQuotas`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceQuotas {
    pub disk: Option<ServiceDiskQuota>,
    pub visits: Option<ServiceVisitsQuota>,
    pub websites: Option<ServiceWebsitesQuota>,
}

/// `managedCMS.ServiceCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCurrentState {
    pub created_at: Option<String>,
    pub dashboards: Option<ServiceDashboards>,
    pub plan: Option<String>,
    pub quotas: Option<ServiceQuotas>,
}

/// `managedCMS.Service`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub checksum: Option<String>,
    pub current_state: Option<ServiceCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
}

/// `managedCMS.ServiceEditionTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEditionTargetSpec {
    pub flush_cdndate: Option<String>,
}

/// `managedCMS.ServiceEditionBody`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEditionBody {
    pub target_spec: Option<ServiceEditionTargetSpec>,
}

/// `managedCMS.ServiceWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceWithIAM {
    pub checksum: Option<String>,
    pub current_state: Option<ServiceCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
}

/// `managedCMS.TaskInputsEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManagedCMSTaskInputs {
    #[serde(rename = "import.cmsSpecific.wordpress.selection")]
    ImportCmsSpecificWordpressSelection,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `managedCMS.UpdateCurrentStateDryRun`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCurrentStateDryRun {
    pub date: Option<String>,
    pub result: Option<String>,
    pub url: Option<String>,
}

/// `managedCMS.UpdateStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManagedCMSUpdateStatus {
    #[serde(rename = "TEST_DOING")]
    TestDoing,
    #[serde(rename = "TEST_DONE")]
    TestDone,
    #[serde(rename = "TEST_FAILED")]
    TestFailed,
    #[serde(rename = "TEST_IMPOSSIBLE")]
    TestImpossible,
    #[serde(rename = "UPDATE_DOING")]
    UpdateDoing,
    #[serde(rename = "UPDATE_DONE")]
    UpdateDone,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `managedCMS.UpdateCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCurrentState {
    pub cms_version: Option<String>,
    pub dry_run: Option<UpdateCurrentStateDryRun>,
    pub planned_at: Option<String>,
    pub runtime_version: Option<String>,
    pub status: Option<ManagedCMSUpdateStatus>,
}

/// `managedCMS.UpdateTargetSpecDryRun`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTargetSpecDryRun {
    pub date: Option<String>,
}

/// `managedCMS.UpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTargetSpec {
    pub cms_version: Option<String>,
    pub dry_run: Option<UpdateTargetSpecDryRun>,
    pub planned_at: Option<String>,
    pub runtime_version: Option<String>,
}

/// `managedCMS.WebsiteImportCheckResult`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteImportCheckResult {
    pub cms_specific: Option<CMSSpecificWebsiteImportCheckResult>,
}

/// `managedCMS.WebsiteCurrentStateImport`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteCurrentStateImport {
    pub check_result: Option<WebsiteImportCheckResult>,
}

/// `managedCMS.WebsiteCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteCurrentState {
    pub cms: Option<ManagedCMSCMS>,
    pub cms_version: Option<String>,
    pub created_at: Option<String>,
    pub default_fqdn: Option<String>,
    pub disk_usage_bytes: Option<i64>,
    pub domains: Option<Vec<Domain>>,
    pub import: Option<WebsiteCurrentStateImport>,
    pub php_version: Option<ManagedCMSPHPVersion>,
    pub planned_update: Option<UpdateCurrentState>,
    pub primary_fqdn: Option<String>,
    pub service_id: Option<String>,
}

/// `managedCMS.WebsiteTargetSpecCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteTargetSpecCreation {
    pub admin_login: Option<String>,
    pub cms: Option<ManagedCMSCMS>,
    pub cms_specific: Option<CMSSpecificWebsiteTargetSpecCreation>,
    pub php_version: Option<ManagedCMSPHPVersion>,
}

/// `managedCMS.WebsiteTargetSpecImport`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteTargetSpecImport {
    pub admin_login: Option<String>,
    pub admin_url: Option<String>,
    pub cms: Option<ManagedCMSCMS>,
    pub cms_specific: Option<CMSSpecificWebsiteTargetSpecImport>,
}

/// `managedCMS.WebsiteTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteTargetSpec {
    pub creation: Option<WebsiteTargetSpecCreation>,
    pub import: Option<WebsiteTargetSpecImport>,
    pub planned_update: Option<UpdateTargetSpec>,
}

/// `managedCMS.Website`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Website {
    pub checksum: Option<String>,
    pub current_state: Option<WebsiteCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<WebsiteTargetSpec>,
}

/// `managedCMS.WebsiteCreationTargetSpecCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteCreationTargetSpecCreation {
    pub admin_login: String,
    #[serde(default)]
    pub admin_password: serde_json::Value,
    pub cms: ManagedCMSCMS,
    pub cms_specific: Option<CMSSpecificWebsiteCreationTargetSpecCreation>,
    pub php_version: Option<ManagedCMSPHPVersion>,
}

/// `managedCMS.WebsiteCreationTargetSpecImport`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteCreationTargetSpecImport {
    pub admin_login: String,
    #[serde(default)]
    pub admin_password: serde_json::Value,
    pub admin_url: String,
    pub cms: ManagedCMSCMS,
}

/// `managedCMS.WebsiteCreationTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteCreationTargetSpec {
    pub creation: Option<WebsiteCreationTargetSpecCreation>,
    pub import: Option<WebsiteCreationTargetSpecImport>,
}

/// `managedCMS.WebsiteCreationBody`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteCreationBody {
    pub target_spec: WebsiteCreationTargetSpec,
}

/// `managedCMS.WebsiteEditionTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteEditionTargetSpec {
    pub domains: Option<Vec<Domain>>,
    pub flush_cdndate: Option<String>,
    pub planned_update: Option<UpdateTargetSpec>,
    pub primary_fqdn: Option<String>,
    pub reset_database_password_date: Option<String>,
}

/// `managedCMS.WebsiteEditionBody`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteEditionBody {
    pub target_spec: Option<WebsiteEditionTargetSpec>,
}

impl OvhClient {
    /// `GET /managedCMS/reference/availableCMS` — List the available content management systems
    pub async fn managed_cms_reference_available_cms(
        &self,
        page: &PageParams,
    ) -> Result<Vec<ManagedCMSCMS>> {
        self.get_page("/managedCMS/reference/availableCMS", &[], page)
            .await
            .map(|p| p.items)
    }

    /// `GET /managedCMS/reference/availableLanguages` — List the available languages when creating a new website
    pub async fn managed_cms_reference_available_languages(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<Language>> {
        self.get_page(
            &Self::append_query("/managedCMS/reference/availableLanguages", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /managedCMS/reference/supportedPHPVersions` — List supported PHP versions
    pub async fn managed_cms_reference_supported_phpversions(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<ManagedCMSPHPVersion>> {
        self.get_page(
            &Self::append_query("/managedCMS/reference/supportedPHPVersions", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /managedCMS/resource` — Get all services of your account
    pub async fn managed_cms_resources(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<ServiceWithIAM>> {
        self.get_page(
            &Self::append_query("/managedCMS/resource", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /managedCMS/resource/{serviceId}` — Get a service
    pub async fn managed_cms_resource(&self, service_id: &str) -> Result<ServiceWithIAM> {
        self.get(&format!(
            "/managedCMS/resource/{}",
            percent_encode(service_id)
        ))
        .await
    }

    /// `PUT /managedCMS/resource/{serviceId}` — Edit a service
    pub async fn managed_cms_resource_put(
        &self,
        service_id: &str,
        body: &ServiceEditionBody,
    ) -> Result<Service> {
        self.put_json(
            &format!("/managedCMS/resource/{}", percent_encode(service_id)),
            body,
        )
        .await
    }

    /// `POST /managedCMS/resource/{serviceId}/flushCDN` — Flush CDN for all websites of the service
    pub async fn managed_cms_resource_flush_cdn_post(
        &self,
        service_id: &str,
    ) -> Result<CurrentTask> {
        self.post_v2_no_body(
            &format!(
                "/managedCMS/resource/{}/flushCDN",
                percent_encode(service_id)
            ),
            V2RequestOptions::default(),
        )
        .await
    }

    /// `GET /managedCMS/resource/{serviceId}/log/kind` — List available log kinds
    pub async fn managed_cms_resource_log_kind(
        &self,
        service_id: &str,
        page: &PageParams,
    ) -> Result<Vec<LogsLogKind>> {
        self.get_page(
            &format!(
                "/managedCMS/resource/{}/log/kind",
                percent_encode(service_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /managedCMS/resource/{serviceId}/log/kind/{name}` — Get a log kind
    pub async fn managed_cms_resource_log_kind_get(
        &self,
        name: &str,
        service_id: &str,
    ) -> Result<LogsLogKind> {
        self.get(&format!(
            "/managedCMS/resource/{}/log/kind/{}",
            percent_encode(service_id),
            percent_encode(name)
        ))
        .await
    }

    /// `GET /managedCMS/resource/{serviceId}/log/subscription` — List subscription IDs for a cluster
    pub async fn managed_cms_resource_log_subscription(
        &self,
        service_id: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<LogsLogSubscription>> {
        self.get_page(
            &Self::append_query(
                &format!(
                    "/managedCMS/resource/{}/log/subscription",
                    percent_encode(service_id)
                ),
                query,
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /managedCMS/resource/{serviceId}/log/subscription` — Create a subscription from logs to a pre-existing LDP stream
    pub async fn managed_cms_resource_log_subscription_post(
        &self,
        service_id: &str,
        body: &LogsLogSubscriptionCreation,
    ) -> Result<LogsLogSubscriptionResponse> {
        self.post_v2(
            &format!(
                "/managedCMS/resource/{}/log/subscription",
                percent_encode(service_id)
            ),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /managedCMS/resource/{serviceId}/log/subscription/{subscriptionId}` — Delete a subscription
    pub async fn managed_cms_resource_log_subscription_delete(
        &self,
        service_id: &str,
        subscription_id: &str,
    ) -> Result<LogsLogSubscriptionResponse> {
        self.delete_json(&format!(
            "/managedCMS/resource/{}/log/subscription/{}",
            percent_encode(service_id),
            percent_encode(subscription_id)
        ))
        .await
    }

    /// `GET /managedCMS/resource/{serviceId}/log/subscription/{subscriptionId}` — Get subscription details
    pub async fn managed_cms_resource_log_subscription_get(
        &self,
        service_id: &str,
        subscription_id: &str,
    ) -> Result<LogsLogSubscription> {
        self.get(&format!(
            "/managedCMS/resource/{}/log/subscription/{}",
            percent_encode(service_id),
            percent_encode(subscription_id)
        ))
        .await
    }

    /// `POST /managedCMS/resource/{serviceId}/log/url` — Generate a temporary URL to retrieve logs
    pub async fn managed_cms_resource_log_url_post(
        &self,
        service_id: &str,
        body: &LogsLogUrlCreation,
    ) -> Result<LogsTemporaryLogsLink> {
        self.post_v2(
            &format!(
                "/managedCMS/resource/{}/log/url",
                percent_encode(service_id)
            ),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `GET /managedCMS/resource/{serviceId}/task` — Get current and recent tasks on the service
    pub async fn managed_cms_resource_task(
        &self,
        service_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Task>> {
        self.get_page(
            &format!("/managedCMS/resource/{}/task", percent_encode(service_id)),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /managedCMS/resource/{serviceId}/task/{taskId}` —
    pub async fn managed_cms_resource_task_get(
        &self,
        service_id: &str,
        task_id: &str,
    ) -> Result<Task> {
        self.get(&format!(
            "/managedCMS/resource/{}/task/{}",
            percent_encode(service_id),
            percent_encode(task_id)
        ))
        .await
    }

    /// `PUT /managedCMS/resource/{serviceId}/task/{taskId}` — Edit a task to provide user input
    pub async fn managed_cms_resource_task_put(
        &self,
        service_id: &str,
        task_id: &str,
        body: &serde_json::Value,
    ) -> Result<Task> {
        self.put_json(
            &format!(
                "/managedCMS/resource/{}/task/{}",
                percent_encode(service_id),
                percent_encode(task_id)
            ),
            body,
        )
        .await
    }

    /// `GET /managedCMS/resource/{serviceId}/website` — Get all websites of a service
    pub async fn managed_cms_resource_website(
        &self,
        service_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Website>> {
        self.get_page(
            &format!(
                "/managedCMS/resource/{}/website",
                percent_encode(service_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /managedCMS/resource/{serviceId}/website` — Create or import a website
    pub async fn managed_cms_resource_website_post(
        &self,
        service_id: &str,
        body: &WebsiteCreationBody,
    ) -> Result<Website> {
        self.post_v2(
            &format!(
                "/managedCMS/resource/{}/website",
                percent_encode(service_id)
            ),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /managedCMS/resource/{serviceId}/website/{websiteId}` — Delete a website
    pub async fn managed_cms_resource_website_delete(
        &self,
        service_id: &str,
        website_id: &str,
    ) -> Result<Website> {
        self.delete_json(&format!(
            "/managedCMS/resource/{}/website/{}",
            percent_encode(service_id),
            percent_encode(website_id)
        ))
        .await
    }

    /// `GET /managedCMS/resource/{serviceId}/website/{websiteId}` — Get a website
    pub async fn managed_cms_resource_website_get(
        &self,
        service_id: &str,
        website_id: &str,
    ) -> Result<Website> {
        self.get(&format!(
            "/managedCMS/resource/{}/website/{}",
            percent_encode(service_id),
            percent_encode(website_id)
        ))
        .await
    }

    /// `PUT /managedCMS/resource/{serviceId}/website/{websiteId}` — Edit a website
    pub async fn managed_cms_resource_website_put(
        &self,
        service_id: &str,
        website_id: &str,
        body: &WebsiteEditionBody,
    ) -> Result<Website> {
        self.put_json(
            &format!(
                "/managedCMS/resource/{}/website/{}",
                percent_encode(service_id),
                percent_encode(website_id)
            ),
            body,
        )
        .await
    }

    /// `POST /managedCMS/resource/{serviceId}/website/{websiteId}/flushCDN` — Flush CDN for the website
    pub async fn managed_cms_resource_website_flush_cdn_post(
        &self,
        service_id: &str,
        website_id: &str,
    ) -> Result<CurrentTask> {
        self.post_v2_no_body(
            &format!(
                "/managedCMS/resource/{}/website/{}/flushCDN",
                percent_encode(service_id),
                percent_encode(website_id)
            ),
            V2RequestOptions::default(),
        )
        .await
    }

    /// `POST /managedCMS/resource/{serviceId}/website/{websiteId}/resetDatabasePassword` — Reset password of the website's database
    pub async fn managed_cms_resource_website_reset_database_password_post(
        &self,
        service_id: &str,
        website_id: &str,
    ) -> Result<CurrentTask> {
        self.post_v2_no_body(
            &format!(
                "/managedCMS/resource/{}/website/{}/resetDatabasePassword",
                percent_encode(service_id),
                percent_encode(website_id)
            ),
            V2RequestOptions::default(),
        )
        .await
    }
}
