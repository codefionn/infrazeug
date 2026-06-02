//! OVHcloud API v2 **vmwareCloudDirector** bindings (`/v2/vmwareCloudDirector`).
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

/// `vmwareCloudDirector.AZNameEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmwareCloudDirectorAZName {
    #[serde(rename = "ca-east-bhs-a")]
    CaEastBhsA,
    #[serde(rename = "eu-central-waw-a")]
    EuCentralWawA,
    #[serde(rename = "eu-west-eri-a")]
    EuWestEriA,
    #[serde(rename = "eu-west-lim-a")]
    EuWestLimA,
    #[serde(rename = "eu-west-rbx-a")]
    EuWestRbxA,
    #[serde(rename = "eu-west-sbg-a")]
    EuWestSbgA,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `vmwareCloudDirector.BackupOfferNameEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmwareCloudDirectorBackupOfferName {
    #[serde(rename = "BRONZE")]
    Bronze,
    #[serde(rename = "GOLD")]
    Gold,
    #[serde(rename = "SILVER")]
    Silver,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `vmwareCloudDirector.BackupResourceStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmwareCloudDirectorBackupResourceStatus {
    #[serde(rename = "CREATING")]
    Creating,
    #[serde(rename = "DISABLED")]
    Disabled,
    #[serde(rename = "DISABLING")]
    Disabling,
    #[serde(rename = "READY")]
    Ready,
    #[serde(rename = "REMOVED")]
    Removed,
    #[serde(rename = "REMOVING")]
    Removing,
    #[serde(rename = "UPDATING")]
    Updating,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `vmwareCloudDirector.BillingTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmwareCloudDirectorBillingType {
    #[serde(rename = "DEMO")]
    Demo,
    #[serde(rename = "MONTHLY")]
    Monthly,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `vmwareCloudDirector.CommercialRangeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmwareCloudDirectorCommercialRange {
    #[serde(rename = "NSX")]
    Nsx,
    #[serde(rename = "STANDARD")]
    Standard,
    #[serde(rename = "VSAN-NSX")]
    VsanNsx,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `vmwareCloudDirector.compute.CurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputeCurrentState {
    pub billing_type: Option<VmwareCloudDirectorBillingType>,
    pub memory_quota: Option<i64>,
    pub name: Option<String>,
    pub profile: Option<String>,
    pub v_cpucount: Option<i64>,
}

/// `vmwareCloudDirector.Compute`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Compute {
    pub current_state: Option<ComputeCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub updated_at: Option<String>,
}

/// `vmwareCloudDirector.networkAcl.Rule`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAclRule {
    pub name: String,
    pub network: String,
}

/// `vmwareCloudDirector.networkAcl.CurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAclCurrentState {
    pub networks: Option<Vec<NetworkAclRule>>,
}

/// `vmwareCloudDirector.networkAcl.TargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAclTargetSpec {
    #[serde(default)]
    pub networks: Vec<NetworkAclRule>,
}

/// `vmwareCloudDirector.NetworkAcl`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAcl {
    pub current_state: Option<NetworkAclCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<NetworkAclTargetSpec>,
    pub updated_at: Option<String>,
}

/// `vmwareCloudDirector.orderableResource.Compute`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderableResourceCompute {
    pub memory_quota: Option<i64>,
    pub name: Option<String>,
    pub profile: Option<String>,
    pub v_cpucount: Option<i64>,
    pub v_cpuspeed: Option<f64>,
}

/// `vmwareCloudDirector.orderableResource.Storage`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderableResourceStorage {
    pub capacity: Option<i64>,
    pub name: Option<String>,
    pub performance_class: Option<i64>,
    pub profile: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

/// `vmwareCloudDirector.OrderableResource`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderableResource {
    pub compute: Option<Vec<OrderableResourceCompute>>,
    pub storage: Option<Vec<OrderableResourceStorage>>,
}

/// `vmwareCloudDirector.RegionNameEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmwareCloudDirectorRegionName {
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
    #[serde(rename = "us-east-vin")]
    UsEastVin,
    #[serde(rename = "us-west-hil")]
    UsWestHil,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `vmwareCloudDirector.organization.CurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationCurrentState {
    pub api_url: Option<String>,
    pub billing_type: Option<VmwareCloudDirectorBillingType>,
    pub description: Option<String>,
    pub full_name: Option<String>,
    pub name: Option<String>,
    pub region: Option<VmwareCloudDirectorRegionName>,
    pub spla: Option<bool>,
    pub web_interface_url: Option<String>,
}

/// `vmwareCloudDirector.organization.TargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationTargetSpec {
    pub description: Option<String>,
    pub full_name: String,
}

/// `vmwareCloudDirector.Organization`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    pub current_state: Option<OrganizationCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<OrganizationTargetSpec>,
    pub updated_at: Option<String>,
}

/// `vmwareCloudDirector.OrganizationWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationWithIAM {
    pub current_state: Option<OrganizationCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<OrganizationTargetSpec>,
    pub updated_at: Option<String>,
}

/// `vmwareCloudDirector.RegionLocationEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmwareCloudDirectorRegionLocation {
    #[serde(rename = "Asia Pacific (Australia - Sydney)")]
    AsiaPacificAustraliaSydney,
    #[serde(rename = "Asia Pacific (Singapore -Singapore)")]
    AsiaPacificSingaporeSingapore,
    #[serde(rename = "Europe (France - Gravelines)")]
    EuropeFranceGravelines,
    #[serde(rename = "Europe (France - Paris)")]
    EuropeFranceParis,
    #[serde(rename = "Europe (France - Roubaix)")]
    EuropeFranceRoubaix,
    #[serde(rename = "Europe (France - Strasbourg)")]
    EuropeFranceStrasbourg,
    #[serde(rename = "Europe (Germany - Limburg)")]
    EuropeGermanyLimburg,
    #[serde(rename = "Europe (Poland - Warsaw)")]
    EuropePolandWarsaw,
    #[serde(rename = "Europe (United Kingdom - Erith)")]
    EuropeUnitedKingdomErith,
    #[serde(rename = "North America (Canada - East - Beauharnois)")]
    NorthAmericaCanadaEastBeauharnois,
    #[serde(rename = "North America (US - East - Vinthill)")]
    NorthAmericaUsEastVinthill,
    #[serde(rename = "North America (US - West - Hillsboro)")]
    NorthAmericaUsWestHillsboro,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `vmwareCloudDirector.Region`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub location: Option<VmwareCloudDirectorRegionLocation>,
    pub region: Option<VmwareCloudDirectorRegionName>,
}

/// `vmwareCloudDirector.storage.CurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageCurrentState {
    pub billing_type: Option<VmwareCloudDirectorBillingType>,
    pub capacity: Option<i64>,
    pub name: Option<String>,
    pub profile: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

/// `vmwareCloudDirector.Storage`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    pub current_state: Option<StorageCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub updated_at: Option<String>,
}

/// `vmwareCloudDirector.virtualDataCenter.CurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualDataCenterCurrentState {
    pub commercial_range: Option<VmwareCloudDirectorCommercialRange>,
    pub description: Option<String>,
    pub ip_quota: Option<i64>,
    pub memory_quota: Option<i64>,
    pub name: Option<String>,
    pub region: Option<VmwareCloudDirectorRegionName>,
    pub storage_quota: Option<i64>,
    pub v_cpucount: Option<i64>,
    pub v_cpuspeed: Option<f64>,
    pub vrack: Option<String>,
}

/// `vmwareCloudDirector.virtualDataCenter.TargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualDataCenterTargetSpec {
    pub description: String,
    pub v_cpuspeed: f64,
}

/// `vmwareCloudDirector.VirtualDataCenter`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualDataCenter {
    pub current_state: Option<VirtualDataCenterCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<VirtualDataCenterTargetSpec>,
    pub updated_at: Option<String>,
}

/// `vmwareCloudDirector.VirtualDataCenterWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualDataCenterWithIAM {
    pub current_state: Option<VirtualDataCenterCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<VirtualDataCenterTargetSpec>,
    pub updated_at: Option<String>,
}

/// `vmwareCloudDirector.VrackSegmentModeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmwareCloudDirectorVrackSegmentMode {
    #[serde(rename = "NSX_EDGE_GATEWAY")]
    NsxEdgeGateway,
    #[serde(rename = "TAGGED")]
    Tagged,
    #[serde(rename = "TRUNK")]
    Trunk,
    #[serde(rename = "UNTAGGED")]
    Untagged,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `vmwareCloudDirector.VrackSegmentTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmwareCloudDirectorVrackSegmentType {
    #[serde(rename = "DEFAULT")]
    Default,
    #[serde(rename = "MIGRATED")]
    Migrated,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `vmwareCloudDirector.vrackSegment.CurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VrackSegmentCurrentState {
    pub mode: Option<VmwareCloudDirectorVrackSegmentMode>,
    pub networks: Option<Vec<String>>,
    #[serde(rename = "type")]
    pub kind: Option<VmwareCloudDirectorVrackSegmentType>,
    pub vlan_id: Option<String>,
}

/// `vmwareCloudDirector.vrackSegment.TargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VrackSegmentTargetSpec {
    pub mode: VmwareCloudDirectorVrackSegmentMode,
    #[serde(default)]
    pub networks: Vec<String>,
    pub vlan_id: String,
}

/// `vmwareCloudDirector.VrackSegment`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VrackSegment {
    pub current_state: Option<VrackSegmentCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<VrackSegmentTargetSpec>,
    pub updated_at: Option<String>,
}

/// `vmwareCloudDirector.backup.CurrentStateOffer`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCurrentStateOffer {
    pub name: Option<VmwareCloudDirectorBackupOfferName>,
    pub protection_primary_region: Option<VmwareCloudDirectorRegionName>,
    pub protection_replicated_region: Option<VmwareCloudDirectorRegionName>,
    pub quota_in_tb: Option<i64>,
    pub status: Option<VmwareCloudDirectorBackupResourceStatus>,
    pub used_space_in_gb: Option<f64>,
}

/// `vmwareCloudDirector.backup.CurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCurrentState {
    pub az_name: Option<VmwareCloudDirectorAZName>,
    pub offers: Option<Vec<BackupCurrentStateOffer>>,
}

/// `vmwareCloudDirector.backup.TargetOffer`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupTargetOffer {
    pub name: VmwareCloudDirectorBackupOfferName,
    pub quota_in_tb: i64,
}

/// `vmwareCloudDirector.backup.TargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupTargetSpec {
    #[serde(default)]
    pub offers: Vec<BackupTargetOffer>,
}

/// `vmwareCloudDirector.backup.BackupDetails`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupBackupDetails {
    pub created_at: Option<String>,
    pub current_state: Option<BackupCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<VmwareCloudDirectorBackupResourceStatus>,
    pub target_spec: Option<BackupTargetSpec>,
    pub updated_at: Option<String>,
}

/// `vmwareCloudDirector.backup.BackupDetailsWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupBackupDetailsWithIAM {
    pub created_at: Option<String>,
    pub current_state: Option<BackupCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub id: Option<String>,
    pub resource_status: Option<VmwareCloudDirectorBackupResourceStatus>,
    pub target_spec: Option<BackupTargetSpec>,
    pub updated_at: Option<String>,
}

/// `vmwareCloudDirector.backup.backupDetails.Update`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupBackupDetailsUpdate {
    pub target_spec: BackupTargetSpec,
}

/// `vmwareCloudDirector.networkAcl.Update`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAclUpdate {
    pub target_spec: NetworkAclTargetSpec,
}

/// `vmwareCloudDirector.organization.Update`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationUpdate {
    pub target_spec: OrganizationTargetSpec,
}

/// `vmwareCloudDirector.virtualDataCenter.Update`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualDataCenterUpdate {
    pub target_spec: VirtualDataCenterTargetSpec,
}

/// `vmwareCloudDirector.vrackSegment.Update`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VrackSegmentUpdate {
    pub target_spec: VrackSegmentTargetSpec,
}

impl OvhClient {
    /// `GET /vmwareCloudDirector/backup` — List VMware Cloud Director Backup services
    pub async fn vmware_cloud_director_backups(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<BackupBackupDetailsWithIAM>> {
        self.get_page(
            &Self::append_query("/vmwareCloudDirector/backup", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /vmwareCloudDirector/backup/{backupId}` — Get VMware Cloud Director Backup service
    pub async fn vmware_cloud_director_backup(
        &self,
        backup_id: &str,
    ) -> Result<BackupBackupDetailsWithIAM> {
        self.get(&format!(
            "/vmwareCloudDirector/backup/{}",
            percent_encode(backup_id)
        ))
        .await
    }

    /// `PUT /vmwareCloudDirector/backup/{backupId}` — Update VMware Cloud Director Backup service
    pub async fn vmware_cloud_director_backup_put(
        &self,
        backup_id: &str,
        body: &BackupBackupDetailsUpdate,
    ) -> Result<BackupBackupDetails> {
        self.put_json(
            &format!("/vmwareCloudDirector/backup/{}", percent_encode(backup_id)),
            body,
        )
        .await
    }

    /// `GET /vmwareCloudDirector/backup/{backupId}/task` — List all asynchronous operations related to the VMware Cloud Director backup service
    pub async fn vmware_cloud_director_backup_task(
        &self,
        backup_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Task>> {
        self.get_page(
            &format!(
                "/vmwareCloudDirector/backup/{}/task",
                percent_encode(backup_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /vmwareCloudDirector/backup/{backupId}/task/{taskId}` — Get a specific task related to the VMware Cloud Director backup service
    pub async fn vmware_cloud_director_backup_task_get(
        &self,
        backup_id: &str,
        task_id: &str,
    ) -> Result<Task> {
        self.get(&format!(
            "/vmwareCloudDirector/backup/{}/task/{}",
            percent_encode(backup_id),
            percent_encode(task_id)
        ))
        .await
    }

    /// `GET /vmwareCloudDirector/organization` — List VMware Cloud Director organizations
    pub async fn vmware_cloud_director_organizations(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<OrganizationWithIAM>> {
        self.get_page(
            &Self::append_query("/vmwareCloudDirector/organization", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}` — Get VMware Cloud Director organization details
    pub async fn vmware_cloud_director_organization(
        &self,
        organization_id: &str,
    ) -> Result<OrganizationWithIAM> {
        self.get(&format!(
            "/vmwareCloudDirector/organization/{}",
            percent_encode(organization_id)
        ))
        .await
    }

    /// `PUT /vmwareCloudDirector/organization/{organizationId}` — Update VMware Cloud Director organization details
    pub async fn vmware_cloud_director_organization_put(
        &self,
        organization_id: &str,
        body: &OrganizationUpdate,
    ) -> Result<Organization> {
        self.put_json(
            &format!(
                "/vmwareCloudDirector/organization/{}",
                percent_encode(organization_id)
            ),
            body,
        )
        .await
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/networkAcl` — List organization network access control list resources
    pub async fn vmware_cloud_director_organization_network_acl(
        &self,
        organization_id: &str,
        page: &PageParams,
    ) -> Result<Vec<NetworkAcl>> {
        self.get_page(
            &format!(
                "/vmwareCloudDirector/organization/{}/networkAcl",
                percent_encode(organization_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/networkAcl/{id}` — Get organization network access control list resources
    pub async fn vmware_cloud_director_organization_network_acl_get(
        &self,
        id: &str,
        organization_id: &str,
    ) -> Result<NetworkAcl> {
        self.get(&format!(
            "/vmwareCloudDirector/organization/{}/networkAcl/{}",
            percent_encode(organization_id),
            percent_encode(id)
        ))
        .await
    }

    /// `PUT /vmwareCloudDirector/organization/{organizationId}/networkAcl/{id}` — Update organization network access control list resources
    pub async fn vmware_cloud_director_organization_network_acl_put(
        &self,
        id: &str,
        organization_id: &str,
        body: &NetworkAclUpdate,
    ) -> Result<NetworkAcl> {
        self.put_json(
            &format!(
                "/vmwareCloudDirector/organization/{}/networkAcl/{}",
                percent_encode(organization_id),
                percent_encode(id)
            ),
            body,
        )
        .await
    }

    /// `POST /vmwareCloudDirector/organization/{organizationId}/password` — Reset the VMware Cloud Director organization administrator password
    pub async fn vmware_cloud_director_organization_password_post(
        &self,
        organization_id: &str,
    ) -> Result<()> {
        self.post_v2_no_body_void(
            &format!(
                "/vmwareCloudDirector/organization/{}/password",
                percent_encode(organization_id)
            ),
            V2RequestOptions::default(),
        )
        .await
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/task` — List all asynchronous operations related to the VMware Cloud Director resources
    pub async fn vmware_cloud_director_organization_task(
        &self,
        organization_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Task>> {
        self.get_page(
            &format!(
                "/vmwareCloudDirector/organization/{}/task",
                percent_encode(organization_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/task/{taskId}` — Get a specific task related to the VMware Cloud Director resources
    pub async fn vmware_cloud_director_organization_task_get(
        &self,
        organization_id: &str,
        task_id: &str,
    ) -> Result<Task> {
        self.get(&format!(
            "/vmwareCloudDirector/organization/{}/task/{}",
            percent_encode(organization_id),
            percent_encode(task_id)
        ))
        .await
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter` — List all organization Virtual DataCenters
    pub async fn vmware_cloud_director_organization_virtual_data_center(
        &self,
        organization_id: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<VirtualDataCenterWithIAM>> {
        self.get_page(
            &Self::append_query(
                &format!(
                    "/vmwareCloudDirector/organization/{}/virtualDataCenter",
                    percent_encode(organization_id)
                ),
                query,
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}` — Get organization Virtual DataCenter details
    pub async fn vmware_cloud_director_organization_virtual_data_center_get(
        &self,
        organization_id: &str,
        virtual_data_center_id: &str,
    ) -> Result<VirtualDataCenterWithIAM> {
        self.get(&format!(
            "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}",
            percent_encode(organization_id),
            percent_encode(virtual_data_center_id)
        ))
        .await
    }

    /// `PUT /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}` — Update organization Virtual DataCenter details
    pub async fn vmware_cloud_director_organization_virtual_data_center_put(
        &self,
        organization_id: &str,
        virtual_data_center_id: &str,
        body: &VirtualDataCenterUpdate,
    ) -> Result<VirtualDataCenter> {
        self.put_json(
            &format!(
                "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}",
                percent_encode(organization_id),
                percent_encode(virtual_data_center_id)
            ),
            body,
        )
        .await
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/compute` — List organization Virtual DataCenter associated compute resources
    pub async fn vmware_cloud_director_organization_virtual_data_center_compute(
        &self,
        organization_id: &str,
        virtual_data_center_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Compute>> {
        self.get_page(
            &format!(
                "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/compute",
                percent_encode(organization_id),
                percent_encode(virtual_data_center_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `DELETE /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/compute/{computeId}` — Delete compute resources associated with an organization's Virtual DataCenter
    pub async fn vmware_cloud_director_organization_virtual_data_center_compute_delete(
        &self,
        compute_id: &str,
        organization_id: &str,
        virtual_data_center_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/compute/{}",
            percent_encode(organization_id),
            percent_encode(virtual_data_center_id),
            percent_encode(compute_id)
        ))
        .await
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/compute/{computeId}` — Get organization Virtual DataCenter associated compute resources
    pub async fn vmware_cloud_director_organization_virtual_data_center_compute_get(
        &self,
        compute_id: &str,
        organization_id: &str,
        virtual_data_center_id: &str,
    ) -> Result<Compute> {
        self.get(&format!(
            "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/compute/{}",
            percent_encode(organization_id),
            percent_encode(virtual_data_center_id),
            percent_encode(compute_id)
        ))
        .await
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/orderableResource` — List all orderable resources related to the organization Virtual DataCenter
    pub async fn vmware_cloud_director_organization_virtual_data_center_orderable_resource(
        &self,
        organization_id: &str,
        virtual_data_center_id: &str,
    ) -> Result<OrderableResource> {
        self.get(&format!(
            "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/orderableResource",
            percent_encode(organization_id),
            percent_encode(virtual_data_center_id)
        ))
        .await
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/storage` — List organization Virtual DataCenter associated storage resources
    pub async fn vmware_cloud_director_organization_virtual_data_center_storage(
        &self,
        organization_id: &str,
        virtual_data_center_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Storage>> {
        self.get_page(
            &format!(
                "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/storage",
                percent_encode(organization_id),
                percent_encode(virtual_data_center_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `DELETE /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/storage/{storageId}` — Delete organization Virtual DataCenter storage resource
    pub async fn vmware_cloud_director_organization_virtual_data_center_storage_delete(
        &self,
        organization_id: &str,
        storage_id: &str,
        virtual_data_center_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/storage/{}",
            percent_encode(organization_id),
            percent_encode(virtual_data_center_id),
            percent_encode(storage_id)
        ))
        .await
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/storage/{storageId}` — Get organization Virtual DataCenter associated storage resources
    pub async fn vmware_cloud_director_organization_virtual_data_center_storage_get(
        &self,
        organization_id: &str,
        storage_id: &str,
        virtual_data_center_id: &str,
    ) -> Result<Storage> {
        self.get(&format!(
            "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/storage/{}",
            percent_encode(organization_id),
            percent_encode(virtual_data_center_id),
            percent_encode(storage_id)
        ))
        .await
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/task` — List all asynchronous operations related to the organization Virtual DataCenter resource
    pub async fn vmware_cloud_director_organization_virtual_data_center_task(
        &self,
        organization_id: &str,
        virtual_data_center_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Task>> {
        self.get_page(
            &format!(
                "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/task",
                percent_encode(organization_id),
                percent_encode(virtual_data_center_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/task/{taskId}` — Get a specific task related to the organization Virtual DataCenter resource
    pub async fn vmware_cloud_director_organization_virtual_data_center_task_get(
        &self,
        organization_id: &str,
        task_id: &str,
        virtual_data_center_id: &str,
    ) -> Result<Task> {
        self.get(&format!(
            "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/task/{}",
            percent_encode(organization_id),
            percent_encode(virtual_data_center_id),
            percent_encode(task_id)
        ))
        .await
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/vrackSegment` — List organization Virtual DataCenter associated vrack segment resources
    pub async fn vmware_cloud_director_organization_virtual_data_center_vrack_segment(
        &self,
        organization_id: &str,
        virtual_data_center_id: &str,
        page: &PageParams,
    ) -> Result<Vec<VrackSegment>> {
        self.get_page(
            &format!(
                "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/vrackSegment",
                percent_encode(organization_id),
                percent_encode(virtual_data_center_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `DELETE /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/vrackSegment/{id}` — Delete VMware Cloud Director vrack segment resources
    pub async fn vmware_cloud_director_organization_virtual_data_center_vrack_segment_delete(
        &self,
        id: &str,
        organization_id: &str,
        virtual_data_center_id: &str,
    ) -> Result<VrackSegment> {
        self.delete_json(&format!(
            "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/vrackSegment/{}",
            percent_encode(organization_id),
            percent_encode(virtual_data_center_id),
            percent_encode(id)
        ))
        .await
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/vrackSegment/{id}` — Get organization Virtual DataCenter associated vrack segment resources
    pub async fn vmware_cloud_director_organization_virtual_data_center_vrack_segment_get(
        &self,
        id: &str,
        organization_id: &str,
        virtual_data_center_id: &str,
    ) -> Result<VrackSegment> {
        self.get(&format!(
            "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/vrackSegment/{}",
            percent_encode(organization_id),
            percent_encode(virtual_data_center_id),
            percent_encode(id)
        ))
        .await
    }

    /// `PUT /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/vrackSegment/{id}` — Update VMware Cloud Director vrack segment resources
    pub async fn vmware_cloud_director_organization_virtual_data_center_vrack_segment_put(
        &self,
        id: &str,
        organization_id: &str,
        virtual_data_center_id: &str,
        body: &VrackSegmentUpdate,
    ) -> Result<VrackSegment> {
        self.put_json(
            &format!(
                "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/vrackSegment/{}",
                percent_encode(organization_id),
                percent_encode(virtual_data_center_id),
                percent_encode(id)
            ),
            body,
        )
        .await
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/vrackSegment/{id}/task` — List all asynchronous operations related to the organization Virtual DataCenter vRack segment resource
    pub async fn vmware_cloud_director_organization_virtual_data_center_vrack_segment_task(
        &self,
        id: &str,
        organization_id: &str,
        virtual_data_center_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Task>> {
        self.get_page(
            &format!(
                "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/vrackSegment/{}/task",
                percent_encode(organization_id),
                percent_encode(virtual_data_center_id),
                percent_encode(id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /vmwareCloudDirector/organization/{organizationId}/virtualDataCenter/{virtualDataCenterId}/vrackSegment/{id}/task/{taskId}` — Get a specific task related to the organization Virtual DataCenter vRack segment resource
    pub async fn vmware_cloud_director_organization_virtual_data_center_vrack_segment_task_get(
        &self,
        id: &str,
        organization_id: &str,
        task_id: &str,
        virtual_data_center_id: &str,
    ) -> Result<Task> {
        self.get(&format!(
            "/vmwareCloudDirector/organization/{}/virtualDataCenter/{}/vrackSegment/{}/task/{}",
            percent_encode(organization_id),
            percent_encode(virtual_data_center_id),
            percent_encode(id),
            percent_encode(task_id)
        ))
        .await
    }

    /// `GET /vmwareCloudDirector/reference/region` — Get region details
    pub async fn vmware_cloud_director_reference_regions(
        &self,
        page: &PageParams,
    ) -> Result<Vec<Region>> {
        self.get_page("/vmwareCloudDirector/reference/region", &[], page)
            .await
            .map(|p| p.items)
    }
}
