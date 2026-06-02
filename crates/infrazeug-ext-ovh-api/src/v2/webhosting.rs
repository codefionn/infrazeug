//! OVHcloud API v2 **webhosting** bindings (`/v2/webhosting`).
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

/// `webhosting.AttachedDomainHosting`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachedDomainHosting {
    pub boost_offer: Option<String>,
    pub display_name: Option<String>,
    pub offer: Option<String>,
    pub service_name: Option<String>,
}

/// `webhosting.SslStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhostingSslStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "NONE")]
    None,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `webhosting.AttachedDomainSsl`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachedDomainSsl {
    pub status: Option<WebhostingSslStatus>,
}

/// `webhosting.CdnStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhostingCdnStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "NONE")]
    None,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `webhosting.CdnStatus`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdnStatus {
    pub status: WebhostingCdnStatus,
}

/// `webhosting.FirewallStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhostingFirewallStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "NONE")]
    None,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `webhosting.FirewallStatus`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirewallStatus {
    pub status: WebhostingFirewallStatus,
}

/// `webhosting.GitStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhostingGitStatus {
    #[serde(rename = "CREATED")]
    Created,
    #[serde(rename = "CREATING")]
    Creating,
    #[serde(rename = "DELETING")]
    Deleting,
    #[serde(rename = "DEPLOYING")]
    Deploying,
    #[serde(rename = "DISABLED")]
    Disabled,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "INITIAL_ERROR")]
    InitialError,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `webhosting.Git`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Git {
    pub status: Option<WebhostingGitStatus>,
    pub vcs_branch: Option<String>,
    pub vcs_url: Option<String>,
}

/// `webhosting.AttachedDomainCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachedDomainCurrentState {
    pub cdn: Option<CdnStatus>,
    pub firewall: Option<FirewallStatus>,
    pub fqdn: Option<String>,
    pub git: Option<Git>,
    pub hosting: Option<AttachedDomainHosting>,
    pub is_default: Option<bool>,
    pub own_log: Option<String>,
    pub path: Option<String>,
    pub ssl: Option<AttachedDomainSsl>,
}

/// `webhosting.AttachedDomain`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachedDomain {
    pub checksum: Option<String>,
    pub current_state: Option<AttachedDomainCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
}

/// `webhosting.DatabaseConfiguration`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConfiguration {
    pub database_name: String,
    pub password: String,
    pub port: i64,
    pub server: String,
    pub user: String,
}

/// `webhosting.DomainCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainCurrentState {
    pub cdn: Option<CdnStatus>,
    pub firewall: Option<FirewallStatus>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub website_id: Option<String>,
}

/// `webhosting.Domain`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Domain {
    pub checksum: Option<String>,
    pub current_state: Option<DomainCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
}

/// `webhosting.IpLocationCountryEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhostingIpLocationCountry {
    #[serde(rename = "BE")]
    Be,
    #[serde(rename = "CA")]
    Ca,
    #[serde(rename = "CZ")]
    Cz,
    #[serde(rename = "DE")]
    De,
    #[serde(rename = "ES")]
    Es,
    #[serde(rename = "FI")]
    Fi,
    #[serde(rename = "FR")]
    Fr,
    #[serde(rename = "IE")]
    Ie,
    #[serde(rename = "IT")]
    It,
    #[serde(rename = "LT")]
    Lt,
    #[serde(rename = "NL")]
    Nl,
    #[serde(rename = "PL")]
    Pl,
    #[serde(rename = "PT")]
    Pt,
    #[serde(rename = "UK")]
    Uk,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `webhosting.ModuleNameEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhostingModuleName {
    #[serde(rename = "DRUPAL")]
    Drupal,
    #[serde(rename = "JOOMLA")]
    Joomla,
    #[serde(rename = "PRESTASHOP")]
    Prestashop,
    #[serde(rename = "WORDPRESS")]
    Wordpress,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `webhosting.Module`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    pub name: WebhostingModuleName,
}

/// `webhosting.ModuleLanguageEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhostingModuleLanguage {
    #[serde(rename = "cz")]
    Cz,
    #[serde(rename = "de")]
    De,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "es")]
    Es,
    #[serde(rename = "fi")]
    Fi,
    #[serde(rename = "fr")]
    Fr,
    #[serde(rename = "it")]
    It,
    #[serde(rename = "lt")]
    Lt,
    #[serde(rename = "nl")]
    Nl,
    #[serde(rename = "pl")]
    Pl,
    #[serde(rename = "pt")]
    Pt,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `webhosting.ModuleAdminConfiguration`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleAdminConfiguration {
    pub admin_login: String,
    pub admin_password: String,
    pub domain: String,
    pub install_path: Option<String>,
    pub language: WebhostingModuleLanguage,
}

/// `webhosting.ModuleStatus`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleStatus {
    pub enabled: Option<bool>,
}

/// `webhosting.SSLCertificateTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhostingSSLCertificateType {
    #[serde(rename = "COMODO")]
    Comodo,
    #[serde(rename = "CUSTOM")]
    Custom,
    #[serde(rename = "LETSENCRYPT")]
    Letsencrypt,
    #[serde(rename = "SECTIGO")]
    Sectigo,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `webhosting.SSLStateEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhostingSSLState {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "CREATING")]
    Creating,
    #[serde(rename = "DELETING")]
    Deleting,
    #[serde(rename = "EXPIRED")]
    Expired,
    #[serde(rename = "IMPORTING")]
    Importing,
    #[serde(rename = "REGENERATING")]
    Regenerating,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `webhosting.SSLCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SSLCurrentState {
    pub additional_domains: Option<Vec<String>>,
    pub certificate_type: Option<WebhostingSSLCertificateType>,
    pub created_at: Option<String>,
    pub expired_at: Option<String>,
    pub main_domain: Option<String>,
    pub state: Option<WebhostingSSLState>,
}

/// `webhosting.SSL`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SSL {
    pub checksum: Option<String>,
    pub current_state: Option<SSLCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
}

/// `webhosting.UserSSHStateEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhostingUserSSHState {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "SFTP_ONLY")]
    SftpOnly,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `webhosting.UserStateEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebhostingUserState {
    #[serde(rename = "OFF")]
    Off,
    #[serde(rename = "RW")]
    Rw,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `webhosting.UserCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserCurrentState {
    pub home: Option<String>,
    pub login: Option<String>,
    pub ssh_state: Option<WebhostingUserSSHState>,
    pub state: Option<WebhostingUserState>,
}

/// `webhosting.UserTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserTargetSpec {
    pub home: Option<String>,
    pub ssh_state: Option<WebhostingUserSSHState>,
    pub state: Option<WebhostingUserState>,
}

/// `webhosting.User`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub checksum: Option<String>,
    pub current_state: Option<UserCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<UserTargetSpec>,
}

/// `webhosting.UserPatchTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPatchTargetSpec {
    pub home: Option<String>,
    pub password: Option<String>,
    pub ssh_state: Option<WebhostingUserSSHState>,
    pub state: Option<WebhostingUserState>,
}

/// `webhosting.UserPatchPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPatchPayload {
    pub target_spec: Option<UserPatchTargetSpec>,
}

/// `webhosting.UserPostTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPostTargetSpec {
    pub home: String,
    pub login: String,
    pub password: String,
    pub ssh_state: WebhostingUserSSHState,
}

/// `webhosting.UserPostPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPostPayload {
    pub target_spec: UserPostTargetSpec,
}

/// `webhosting.UserPutTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPutTargetSpec {
    pub home: String,
    pub password: String,
    pub ssh_state: WebhostingUserSSHState,
    pub state: WebhostingUserState,
}

/// `webhosting.UserPutPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPutPayload {
    pub target_spec: UserPutTargetSpec,
}

/// `webhosting.WebCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebCurrentState {
    pub name: Option<String>,
}

/// `webhosting.Web`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Web {
    pub checksum: Option<String>,
    pub current_state: Option<WebCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
}

/// `webhosting.WebWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebWithIAM {
    pub checksum: Option<String>,
    pub current_state: Option<WebCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
}

/// `webhosting.WebsiteCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteCurrentState {
    pub git: Option<Git>,
    pub linked_domains: Option<i64>,
    pub module: Option<ModuleStatus>,
    pub name: Option<String>,
    pub path: Option<String>,
}

/// `webhosting.WebsiteTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteTargetSpec {
    pub name: String,
    pub path: Option<String>,
}

/// `webhosting.Website`
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

/// `webhosting.WebsitePostTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsitePostTargetSpec {
    pub admin_configuration: Option<ModuleAdminConfiguration>,
    pub bypass_dnsconfiguration: Option<bool>,
    pub cdn: Option<CdnStatus>,
    pub database_configuration: Option<DatabaseConfiguration>,
    pub firewall: Option<FirewallStatus>,
    pub fqdn: String,
    pub ip_location: Option<WebhostingIpLocationCountry>,
    pub module: Option<Module>,
    pub name: String,
    pub path: Option<String>,
}

/// `webhosting.WebsitePostPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsitePostPayload {
    pub target_spec: WebsitePostTargetSpec,
}

/// `webhosting.WebsitePutPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebsitePutPayload {
    pub target_spec: WebsiteTargetSpec,
}

impl OvhClient {
    /// `GET /webhosting/attachedDomain` —
    pub async fn webhosting_attached_domains(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<AttachedDomain>> {
        self.get_page(
            &Self::append_query("/webhosting/attachedDomain", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /webhosting/resource` —
    pub async fn webhosting_resources(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<WebWithIAM>> {
        self.get_page(
            &Self::append_query("/webhosting/resource", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /webhosting/resource/{name}` —
    pub async fn webhosting_resource(&self, name: &str) -> Result<WebWithIAM> {
        self.get(&format!("/webhosting/resource/{}", percent_encode(name)))
            .await
    }

    /// `GET /webhosting/resource/{name}/attachedDomain` —
    pub async fn webhosting_resource_attached_domain(
        &self,
        name: &str,
        page: &PageParams,
    ) -> Result<Vec<AttachedDomain>> {
        self.get_page(
            &format!(
                "/webhosting/resource/{}/attachedDomain",
                percent_encode(name)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /webhosting/resource/{name}/certificate` —
    pub async fn webhosting_resource_certificate(
        &self,
        name: &str,
        page: &PageParams,
    ) -> Result<Vec<SSL>> {
        self.get_page(
            &format!("/webhosting/resource/{}/certificate", percent_encode(name)),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /webhosting/resource/{name}/website` —
    pub async fn webhosting_resource_website(
        &self,
        name: &str,
        page: &PageParams,
    ) -> Result<Vec<Website>> {
        self.get_page(
            &format!("/webhosting/resource/{}/website", percent_encode(name)),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /webhosting/resource/{name}/website` — Create a website
    pub async fn webhosting_resource_website_post(
        &self,
        name: &str,
        body: &WebsitePostPayload,
    ) -> Result<Website> {
        self.post_v2(
            &format!("/webhosting/resource/{}/website", percent_encode(name)),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `GET /webhosting/resource/{name}/website/{websiteId}` —
    pub async fn webhosting_resource_website_get(
        &self,
        name: &str,
        website_id: &str,
    ) -> Result<Website> {
        self.get(&format!(
            "/webhosting/resource/{}/website/{}",
            percent_encode(name),
            percent_encode(website_id)
        ))
        .await
    }

    /// `PUT /webhosting/resource/{name}/website/{websiteId}` — Update an existing website
    pub async fn webhosting_resource_website_put(
        &self,
        name: &str,
        website_id: &str,
        body: &WebsitePutPayload,
    ) -> Result<Website> {
        self.put_json(
            &format!(
                "/webhosting/resource/{}/website/{}",
                percent_encode(name),
                percent_encode(website_id)
            ),
            body,
        )
        .await
    }

    /// `GET /webhosting/resource/{name}/website/{websiteId}/domain` —
    pub async fn webhosting_resource_website_domain(
        &self,
        name: &str,
        website_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Domain>> {
        self.get_page(
            &format!(
                "/webhosting/resource/{}/website/{}/domain",
                percent_encode(name),
                percent_encode(website_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }
}
