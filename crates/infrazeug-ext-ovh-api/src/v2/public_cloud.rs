//! OVHcloud API v2 **publicCloud** bindings (`/v2/publicCloud`).
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

/// `publicCloud.common.Location`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonLocation {
    pub availability_zone: Option<String>,
    pub region: String,
}

/// `publicCloud.blockStorage.BackupCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBackupCurrentState {
    pub description: Option<String>,
    pub location: Option<CommonLocation>,
    pub name: Option<String>,
    pub size: Option<i64>,
    pub volume_id: Option<String>,
}

/// `publicCloud.blockStorage.BackupTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBackupTargetSpec {
    pub description: Option<String>,
    pub location: CommonLocation,
    pub name: String,
    pub volume_id: String,
}

/// `publicCloud.blockStorage.Backup`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBackup {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<BlockStorageBackupCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<BlockStorageBackupTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.blockStorage.BackupCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBackupCreation {
    pub target_spec: BlockStorageBackupTargetSpec,
}

/// `publicCloud.blockStorage.BackupUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBackupUpdateTargetSpec {
    pub description: Option<String>,
    pub name: Option<String>,
}

/// `publicCloud.blockStorage.BackupUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBackupUpdate {
    pub checksum: String,
    pub target_spec: BlockStorageBackupUpdateTargetSpec,
}

/// `publicCloud.blockStorage.BlockAttachedInstance`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBlockAttachedInstance {
    pub id: Option<String>,
}

/// `publicCloud.blockStorage.BlockEncryption`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBlockEncryption {
    pub enabled: bool,
}

/// `publicCloud.blockStorage.BlockLocation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBlockLocation {
    pub availability_zone: Option<String>,
    pub region: String,
}

/// `publicCloud.blockStorage.VolumeStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudBlockStorageVolumeStatus {
    #[serde(rename = "ATTACHING")]
    Attaching,
    #[serde(rename = "AVAILABLE")]
    Available,
    #[serde(rename = "CREATING")]
    Creating,
    #[serde(rename = "DELETING")]
    Deleting,
    #[serde(rename = "DETACHING")]
    Detaching,
    #[serde(rename = "DOWNLOADING")]
    Downloading,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "ERROR_BACKING_UP")]
    ErrorBackingUp,
    #[serde(rename = "ERROR_DELETING")]
    ErrorDeleting,
    #[serde(rename = "ERROR_EXTENDING")]
    ErrorExtending,
    #[serde(rename = "ERROR_RESTORING")]
    ErrorRestoring,
    #[serde(rename = "EXTENDING")]
    Extending,
    #[serde(rename = "IN_USE")]
    InUse,
    #[serde(rename = "RETYPING")]
    Retyping,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.blockStorage.VolumeTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudBlockStorageVolumeType {
    #[serde(rename = "CLASSIC")]
    Classic,
    #[serde(rename = "HIGH_SPEED")]
    HighSpeed,
    #[serde(rename = "HIGH_SPEED_GEN2")]
    HighSpeedGen2,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.blockStorage.BlockCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBlockCurrentState {
    pub attached_instances: Option<Vec<BlockStorageBlockAttachedInstance>>,
    pub bootable: Option<bool>,
    pub description: Option<String>,
    pub encryption: Option<BlockStorageBlockEncryption>,
    pub location: Option<BlockStorageBlockLocation>,
    pub locked: Option<bool>,
    pub name: Option<String>,
    pub size: Option<i64>,
    pub status: Option<PublicCloudBlockStorageVolumeStatus>,
    pub volume_type: Option<PublicCloudBlockStorageVolumeType>,
}

/// `publicCloud.blockStorage.BlockCreateFrom`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBlockCreateFrom {
    pub backup_id: Option<String>,
    pub image_id: Option<String>,
    pub snapshot_id: Option<String>,
}

/// `publicCloud.blockStorage.BlockTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBlockTargetSpec {
    pub create_from: Option<BlockStorageBlockCreateFrom>,
    pub encryption: Option<BlockStorageBlockEncryption>,
    pub location: BlockStorageBlockLocation,
    pub name: String,
    pub size: i64,
    pub volume_type: Option<PublicCloudBlockStorageVolumeType>,
}

/// `publicCloud.blockStorage.Block`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBlock {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<BlockStorageBlockCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<BlockStorageBlockTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.blockStorage.BlockCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBlockCreation {
    pub target_spec: BlockStorageBlockTargetSpec,
}

/// `publicCloud.blockStorage.BlockUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBlockUpdateTargetSpec {
    pub name: String,
    pub size: i64,
    pub volume_type: Option<PublicCloudBlockStorageVolumeType>,
}

/// `publicCloud.blockStorage.BlockUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageBlockUpdate {
    pub checksum: String,
    pub target_spec: BlockStorageBlockUpdateTargetSpec,
}

/// `publicCloud.blockStorage.SnapshotCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageSnapshotCurrentState {
    pub description: Option<String>,
    pub location: Option<CommonLocation>,
    pub name: Option<String>,
    pub size: Option<i64>,
    pub volume_id: Option<String>,
}

/// `publicCloud.blockStorage.SnapshotTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageSnapshotTargetSpec {
    pub description: Option<String>,
    pub location: CommonLocation,
    pub name: String,
    pub volume_id: String,
}

/// `publicCloud.blockStorage.Snapshot`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageSnapshot {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<BlockStorageSnapshotCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<BlockStorageSnapshotTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.blockStorage.SnapshotCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageSnapshotCreation {
    pub target_spec: BlockStorageSnapshotTargetSpec,
}

/// `publicCloud.blockStorage.SnapshotUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageSnapshotUpdateTargetSpec {
    pub description: Option<String>,
    pub name: String,
}

/// `publicCloud.blockStorage.SnapshotUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockStorageSnapshotUpdate {
    pub checksum: String,
    pub target_spec: BlockStorageSnapshotUpdateTargetSpec,
}

/// `publicCloud.floatingIp.FloatingIPLocation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingIpFloatingIPLocation {
    pub availability_zone: Option<String>,
    pub region: String,
}

/// `publicCloud.floatingIp.FloatingIPNetwork`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingIpFloatingIPNetwork {
    pub id: Option<String>,
}

/// `publicCloud.floatingIp.FloatingIPStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudFloatingIpFloatingIPStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "DOWN")]
    Down,
    #[serde(rename = "ERROR")]
    Error,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.floatingIp.FloatingIPCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingIpFloatingIPCurrentState {
    pub description: Option<String>,
    #[serde(default)]
    pub ip: serde_json::Value,
    pub location: Option<FloatingIpFloatingIPLocation>,
    pub network: Option<FloatingIpFloatingIPNetwork>,
    pub status: Option<PublicCloudFloatingIpFloatingIPStatus>,
}

/// `publicCloud.floatingIp.FloatingIPTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingIpFloatingIPTargetSpec {
    pub description: Option<String>,
    pub location: FloatingIpFloatingIPLocation,
}

/// `publicCloud.floatingIp.FloatingIP`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingIpFloatingIP {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<FloatingIpFloatingIPCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<FloatingIpFloatingIPTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.floatingIp.FloatingIPCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingIpFloatingIPCreation {
    pub target_spec: FloatingIpFloatingIPTargetSpec,
}

/// `publicCloud.floatingIp.FloatingIPUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingIpFloatingIPUpdateTargetSpec {
    pub description: String,
}

/// `publicCloud.floatingIp.FloatingIPUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatingIpFloatingIPUpdate {
    pub checksum: String,
    pub target_spec: FloatingIpFloatingIPUpdateTargetSpec,
}

/// `publicCloud.gateway.ExternalGatewayModelEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudGatewayExternalGatewayModel {
    #[serde(rename = "2XL")]
    V2Xl,
    #[serde(rename = "3XL")]
    V3Xl,
    #[serde(rename = "L")]
    L,
    #[serde(rename = "M")]
    M,
    #[serde(rename = "S")]
    S,
    #[serde(rename = "XL")]
    Xl,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.gateway.ExternalGateway`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayExternalGateway {
    pub enabled: bool,
    pub model: Option<PublicCloudGatewayExternalGatewayModel>,
}

/// `publicCloud.gateway.GatewayLocation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayGatewayLocation {
    pub availability_zone: Option<String>,
    pub region: String,
}

/// `publicCloud.gateway.GatewayStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudGatewayGatewayStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "BUILD")]
    Build,
    #[serde(rename = "DOWN")]
    Down,
    #[serde(rename = "ERROR")]
    Error,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.gateway.GatewaySubnet`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayGatewaySubnet {
    pub id: String,
}

/// `publicCloud.gateway.GatewayCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayGatewayCurrentState {
    pub description: Option<String>,
    pub external_gateway: Option<GatewayExternalGateway>,
    #[serde(default)]
    pub external_ip: serde_json::Value,
    pub location: Option<GatewayGatewayLocation>,
    pub name: Option<String>,
    pub status: Option<PublicCloudGatewayGatewayStatus>,
    pub subnets: Option<Vec<GatewayGatewaySubnet>>,
}

/// `publicCloud.gateway.GatewayTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayGatewayTargetSpec {
    pub description: Option<String>,
    pub external_gateway: GatewayExternalGateway,
    pub location: GatewayGatewayLocation,
    pub name: String,
    pub subnets: Option<Vec<GatewayGatewaySubnet>>,
}

/// `publicCloud.gateway.Gateway`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayGateway {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<GatewayGatewayCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<GatewayGatewayTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.gateway.GatewayCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayGatewayCreation {
    pub target_spec: GatewayGatewayTargetSpec,
}

/// `publicCloud.gateway.GatewayUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayGatewayUpdateTargetSpec {
    pub description: Option<String>,
    pub external_gateway: GatewayExternalGateway,
    pub name: String,
    pub subnets: Option<Vec<GatewayGatewaySubnet>>,
}

/// `publicCloud.gateway.GatewayUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayGatewayUpdate {
    pub checksum: String,
    pub target_spec: GatewayGatewayUpdateTargetSpec,
}

/// `publicCloud.instance.AutobackupDistant`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceAutobackupDistant {
    pub image_name: String,
    pub region: String,
}

/// `publicCloud.instance.AutobackupExecutionStateEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudInstanceAutobackupExecutionState {
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "IDLE")]
    Idle,
    #[serde(rename = "RUNNING")]
    Running,
    #[serde(rename = "SUCCESS")]
    Success,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.instance.AutobackupExecution`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceAutobackupExecution {
    pub error_message: Option<String>,
    pub id: Option<String>,
    pub started_at: Option<String>,
    pub state: Option<PublicCloudInstanceAutobackupExecutionState>,
    pub updated_at: Option<String>,
}

/// `publicCloud.instance.AutobackupInstanceRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceAutobackupInstanceRef {
    pub id: String,
}

/// `publicCloud.instance.AutobackupCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceAutobackupCurrentState {
    pub cron: Option<String>,
    pub distant: Option<InstanceAutobackupDistant>,
    pub image_name: Option<String>,
    pub instance: Option<InstanceAutobackupInstanceRef>,
    pub last_executions: Option<Vec<InstanceAutobackupExecution>>,
    pub location: Option<CommonLocation>,
    pub name: Option<String>,
    pub next_execution_time: Option<String>,
    pub rotation: Option<i64>,
    pub workflow_name: Option<String>,
}

/// `publicCloud.instance.AutobackupTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceAutobackupTargetSpec {
    pub cron: String,
    pub distant: Option<InstanceAutobackupDistant>,
    pub image_name: String,
    pub instance: InstanceAutobackupInstanceRef,
    pub location: CommonLocation,
    pub name: String,
    pub rotation: i64,
}

/// `publicCloud.instance.Autobackup`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceAutobackup {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<InstanceAutobackupCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<InstanceAutobackupTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.instance.AutobackupCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceAutobackupCreation {
    pub target_spec: InstanceAutobackupTargetSpec,
}

/// `publicCloud.instance.BackupInstanceRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceBackupInstanceRef {
    pub id: String,
}

/// `publicCloud.instance.ImageStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudInstanceImageStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "DEACTIVATED")]
    Deactivated,
    #[serde(rename = "DELETED")]
    Deleted,
    #[serde(rename = "IMPORTING")]
    Importing,
    #[serde(rename = "KILLED")]
    Killed,
    #[serde(rename = "PENDING_DELETE")]
    PendingDelete,
    #[serde(rename = "QUEUED")]
    Queued,
    #[serde(rename = "SAVING")]
    Saving,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.instance.ImageVisibilityEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudInstanceImageVisibility {
    #[serde(rename = "COMMUNITY")]
    Community,
    #[serde(rename = "PRIVATE")]
    Private,
    #[serde(rename = "PUBLIC")]
    Public,
    #[serde(rename = "SHARED")]
    Shared,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.instance.BackupCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceBackupCurrentState {
    pub instance: Option<InstanceBackupInstanceRef>,
    pub location: Option<CommonLocation>,
    pub min_disk: Option<i64>,
    pub min_ram: Option<i64>,
    pub name: Option<String>,
    pub size: Option<i64>,
    pub status: Option<PublicCloudInstanceImageStatus>,
    pub visibility: Option<PublicCloudInstanceImageVisibility>,
}

/// `publicCloud.instance.BackupTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceBackupTargetSpec {
    pub instance: InstanceBackupInstanceRef,
    pub location: CommonLocation,
    pub name: String,
}

/// `publicCloud.instance.Backup`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceBackup {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<InstanceBackupCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<InstanceBackupTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.instance.BackupCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceBackupCreation {
    pub target_spec: InstanceBackupTargetSpec,
}

/// `publicCloud.instance.ConsoleOutput`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceConsoleOutput {
    pub output: Option<String>,
}

/// `publicCloud.instance.InstanceFlavor`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceFlavor {
    pub disk: Option<i64>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub ram: Option<i64>,
    pub vcpus: Option<i64>,
}

/// `publicCloud.instance.InstanceGroupRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceGroupRef {
    pub id: String,
}

/// `publicCloud.instance.InstanceImage`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceImage {
    pub deprecated: Option<bool>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub size: Option<i64>,
    pub status: Option<PublicCloudInstanceImageStatus>,
}

/// `publicCloud.instance.InstanceLocation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceLocation {
    pub availability_zone: Option<String>,
    pub region: String,
}

/// `publicCloud.instance.InstanceNetworkAddress`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceNetworkAddress {
    #[serde(default)]
    pub ip: serde_json::Value,
    #[serde(default)]
    pub mac: serde_json::Value,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub version: Option<i64>,
}

/// `publicCloud.instance.InstanceNetwork`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceNetwork {
    pub addresses: Option<Vec<InstanceInstanceNetworkAddress>>,
    pub floating_ip_id: Option<String>,
    pub gateway_id: Option<String>,
    pub id: Option<String>,
    pub public: Option<bool>,
    pub subnet_id: Option<String>,
}

/// `publicCloud.instance.InstanceVolume`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceVolume {
    pub id: Option<String>,
    pub name: Option<String>,
    pub size: Option<i64>,
}

/// `publicCloud.instance.PowerStateEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudInstancePowerState {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "RESCUE")]
    Rescue,
    #[serde(rename = "SHELVED")]
    Shelved,
    #[serde(rename = "SHUTOFF")]
    Shutoff,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.securityGroup.SecurityGroupRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupSecurityGroupRef {
    pub id: String,
}

/// `publicCloud.instance.InstanceCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceCurrentState {
    pub flavor: Option<InstanceInstanceFlavor>,
    pub group: Option<InstanceInstanceGroupRef>,
    pub host_id: Option<String>,
    pub image: Option<InstanceInstanceImage>,
    pub location: Option<InstanceInstanceLocation>,
    pub locked: Option<bool>,
    pub name: Option<String>,
    pub networks: Option<Vec<InstanceInstanceNetwork>>,
    pub power_state: Option<PublicCloudInstancePowerState>,
    pub project_id: Option<String>,
    pub security_groups: Option<Vec<SecurityGroupSecurityGroupRef>>,
    pub ssh_key_name: Option<String>,
    pub user_id: Option<String>,
    pub volumes: Option<Vec<InstanceInstanceVolume>>,
}

/// `publicCloud.instance.InstanceFlavorRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceFlavorRef {
    pub id: String,
}

/// `publicCloud.instance.InstanceImageRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceImageRef {
    pub id: String,
}

/// `publicCloud.instance.InstanceNetworkRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceNetworkRef {
    pub floating_ip_id: Option<String>,
    pub id: Option<String>,
    pub public: bool,
    pub subnet_id: Option<String>,
}

/// `publicCloud.instance.InstanceVolumeRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceVolumeRef {
    pub id: String,
}

/// `publicCloud.instance.InstanceTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceTargetSpec {
    pub flavor: InstanceInstanceFlavorRef,
    pub group: Option<InstanceInstanceGroupRef>,
    pub image: InstanceInstanceImageRef,
    pub location: InstanceInstanceLocation,
    pub name: String,
    pub networks: Option<Vec<InstanceInstanceNetworkRef>>,
    pub power_state: Option<PublicCloudInstancePowerState>,
    pub security_groups: Option<Vec<SecurityGroupSecurityGroupRef>>,
    pub ssh_key_name: Option<String>,
    pub volumes: Option<Vec<InstanceInstanceVolumeRef>>,
}

/// `publicCloud.instance.Instance`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstance {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<InstanceInstanceCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<InstanceInstanceTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.instance.InstanceActionParameters`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceActionParameters {
    pub hard: Option<bool>,
    pub image_id: Option<String>,
}

/// `publicCloud.instance.InstanceActionTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudInstanceInstanceActionType {
    #[serde(rename = "LOCK")]
    Lock,
    #[serde(rename = "REBOOT")]
    Reboot,
    #[serde(rename = "RESCUE")]
    Rescue,
    #[serde(rename = "UNLOCK")]
    Unlock,
    #[serde(rename = "UNRESCUE")]
    Unrescue,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.instance.InstanceActionRequest`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceActionRequest {
    pub checksum: String,
    pub parameters: Option<InstanceInstanceActionParameters>,
    #[serde(rename = "type")]
    pub kind: PublicCloudInstanceInstanceActionType,
}

/// `publicCloud.instance.InstanceCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceCreation {
    pub target_spec: InstanceInstanceTargetSpec,
}

/// `publicCloud.instance.InstanceUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceUpdateTargetSpec {
    pub flavor: InstanceInstanceFlavorRef,
    pub image: InstanceInstanceImageRef,
    pub name: String,
    pub networks: Option<Vec<InstanceInstanceNetworkRef>>,
    pub power_state: Option<PublicCloudInstancePowerState>,
    pub security_groups: Option<Vec<SecurityGroupSecurityGroupRef>>,
    pub volumes: Option<Vec<InstanceInstanceVolumeRef>>,
}

/// `publicCloud.instance.InstanceUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInstanceUpdate {
    pub checksum: String,
    pub target_spec: InstanceInstanceUpdateTargetSpec,
}

/// `publicCloud.instance.RemoteConsoleProtocolEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudInstanceRemoteConsoleProtocol {
    #[serde(rename = "SERIAL")]
    Serial,
    #[serde(rename = "SPICE")]
    Spice,
    #[serde(rename = "VNC")]
    Vnc,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.instance.RemoteConsoleTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudInstanceRemoteConsoleType {
    #[serde(rename = "NOVNC")]
    Novnc,
    #[serde(rename = "SERIAL")]
    Serial,
    #[serde(rename = "SPICE_HTML5")]
    SpiceHtml5,
    #[serde(rename = "XVPVNC")]
    Xvpvnc,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.instance.RemoteConsole`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRemoteConsole {
    pub protocol: Option<PublicCloudInstanceRemoteConsoleProtocol>,
    #[serde(rename = "type")]
    pub kind: Option<PublicCloudInstanceRemoteConsoleType>,
    pub url: Option<String>,
}

/// `publicCloud.instance.RemoteConsoleRequest`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRemoteConsoleRequest {
    pub protocol: PublicCloudInstanceRemoteConsoleProtocol,
    #[serde(rename = "type")]
    pub kind: PublicCloudInstanceRemoteConsoleType,
}

/// `publicCloud.instanceGroup.InstanceGroupMemberRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceGroupInstanceGroupMemberRef {
    pub id: Option<String>,
}

/// `publicCloud.instanceGroup.PolicyEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudInstanceGroupPolicy {
    #[serde(rename = "AFFINITY")]
    Affinity,
    #[serde(rename = "ANTI_AFFINITY")]
    AntiAffinity,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.instanceGroup.InstanceGroupCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceGroupInstanceGroupCurrentState {
    pub location: Option<CommonLocation>,
    pub members: Option<Vec<InstanceGroupInstanceGroupMemberRef>>,
    pub name: Option<String>,
    pub policy: Option<PublicCloudInstanceGroupPolicy>,
}

/// `publicCloud.instanceGroup.InstanceGroupTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceGroupInstanceGroupTargetSpec {
    pub location: CommonLocation,
    pub name: String,
    pub policy: PublicCloudInstanceGroupPolicy,
}

/// `publicCloud.instanceGroup.InstanceGroup`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceGroupInstanceGroup {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<InstanceGroupInstanceGroupCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<InstanceGroupInstanceGroupTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.instanceGroup.InstanceGroupCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceGroupInstanceGroupCreation {
    pub target_spec: InstanceGroupInstanceGroupTargetSpec,
}

/// `publicCloud.keyManager.AlgorithmEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudKeyManagerAlgorithm {
    #[serde(rename = "AES")]
    Aes,
    #[serde(rename = "DH")]
    Dh,
    #[serde(rename = "DSA")]
    Dsa,
    #[serde(rename = "EC")]
    Ec,
    #[serde(rename = "RSA")]
    Rsa,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.keyManager.BitLengthEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudKeyManagerBitLength {
    #[serde(rename = "128")]
    V128,
    #[serde(rename = "256")]
    V256,
    #[serde(rename = "512")]
    V512,
    #[serde(rename = "1024")]
    V1024,
    #[serde(rename = "2048")]
    V2048,
    #[serde(rename = "4096")]
    V4096,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.keyManager.ConsumerResourceTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudKeyManagerConsumerResourceType {
    #[serde(rename = "IMAGE")]
    Image,
    #[serde(rename = "INSTANCE")]
    Instance,
    #[serde(rename = "LOADBALANCER")]
    Loadbalancer,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.keyManager.ConsumerServiceEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudKeyManagerConsumerService {
    #[serde(rename = "COMPUTE")]
    Compute,
    #[serde(rename = "IMAGE")]
    Image,
    #[serde(rename = "LOADBALANCER")]
    Loadbalancer,
    #[serde(rename = "NETWORK")]
    Network,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.keyManager.ContainerSecretRefSecret`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerContainerSecretRefSecret {
    pub id: String,
}

/// `publicCloud.keyManager.ContainerSecretRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerContainerSecretRef {
    pub name: String,
    pub secret: KeyManagerContainerSecretRefSecret,
}

/// `publicCloud.keyManager.ContainerStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudKeyManagerContainerStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "ERROR")]
    Error,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.keyManager.ContainerTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudKeyManagerContainerType {
    #[serde(rename = "CERTIFICATE")]
    Certificate,
    #[serde(rename = "GENERIC")]
    Generic,
    #[serde(rename = "RSA")]
    Rsa,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.keyManager.ContainerCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerContainerCurrentState {
    pub creator_id: Option<String>,
    pub location: Option<CommonLocation>,
    pub name: Option<String>,
    pub secret_refs: Option<Vec<KeyManagerContainerSecretRef>>,
    pub status: Option<PublicCloudKeyManagerContainerStatus>,
    #[serde(rename = "type")]
    pub kind: Option<PublicCloudKeyManagerContainerType>,
}

/// `publicCloud.keyManager.ContainerTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerContainerTargetSpec {
    pub location: CommonLocation,
    pub name: String,
    pub secret_refs: Option<Vec<KeyManagerContainerSecretRef>>,
    #[serde(rename = "type")]
    pub kind: PublicCloudKeyManagerContainerType,
}

/// `publicCloud.keyManager.Container`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerContainer {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<KeyManagerContainerCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<KeyManagerContainerTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.keyManager.ContainerConsumer`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerContainerConsumer {
    pub id: Option<String>,
    pub resource_id: Option<String>,
    pub resource_type: Option<PublicCloudKeyManagerConsumerResourceType>,
    pub service: Option<PublicCloudKeyManagerConsumerService>,
}

/// `publicCloud.keyManager.ContainerConsumerInput`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerContainerConsumerInput {
    pub resource_id: String,
    pub resource_type: PublicCloudKeyManagerConsumerResourceType,
    pub service: PublicCloudKeyManagerConsumerService,
}

/// `publicCloud.keyManager.ContainerCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerContainerCreation {
    pub target_spec: KeyManagerContainerTargetSpec,
}

/// `publicCloud.keyManager.ContainerUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerContainerUpdateTargetSpec {
    #[serde(default)]
    pub secret_refs: Vec<KeyManagerContainerSecretRef>,
}

/// `publicCloud.keyManager.ContainerUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerContainerUpdate {
    pub checksum: String,
    pub target_spec: KeyManagerContainerUpdateTargetSpec,
}

/// `publicCloud.keyManager.ModeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudKeyManagerMode {
    #[serde(rename = "CBC")]
    Cbc,
    #[serde(rename = "CTR")]
    Ctr,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.keyManager.PayloadContentTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudKeyManagerPayloadContentType {
    #[serde(rename = "APPLICATION_OCTET_STREAM")]
    ApplicationOctetStream,
    #[serde(rename = "APPLICATION_PKCS8")]
    ApplicationPkcs8,
    #[serde(rename = "APPLICATION_PKIX_CERT")]
    ApplicationPkixCert,
    #[serde(rename = "TEXT_PLAIN")]
    TextPlain,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.keyManager.SecretStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudKeyManagerSecretStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "ERROR")]
    Error,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.keyManager.SecretTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudKeyManagerSecretType {
    #[serde(rename = "CERTIFICATE")]
    Certificate,
    #[serde(rename = "OPAQUE")]
    Opaque,
    #[serde(rename = "PASSPHRASE")]
    Passphrase,
    #[serde(rename = "PRIVATE")]
    Private,
    #[serde(rename = "PUBLIC")]
    Public,
    #[serde(rename = "SYMMETRIC")]
    Symmetric,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.keyManager.SecretCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerSecretCurrentState {
    pub algorithm: Option<PublicCloudKeyManagerAlgorithm>,
    pub bit_length: Option<PublicCloudKeyManagerBitLength>,
    pub creator_id: Option<String>,
    pub expiration: Option<String>,
    pub location: Option<CommonLocation>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub mode: Option<PublicCloudKeyManagerMode>,
    pub name: Option<String>,
    pub payload_content_type: Option<PublicCloudKeyManagerPayloadContentType>,
    pub secret_type: Option<PublicCloudKeyManagerSecretType>,
    pub status: Option<PublicCloudKeyManagerSecretStatus>,
}

/// `publicCloud.keyManager.SecretTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerSecretTargetSpec {
    pub algorithm: Option<PublicCloudKeyManagerAlgorithm>,
    pub bit_length: Option<PublicCloudKeyManagerBitLength>,
    pub expiration: Option<String>,
    pub location: CommonLocation,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub mode: Option<PublicCloudKeyManagerMode>,
    pub name: Option<String>,
    #[serde(default)]
    pub payload: serde_json::Value,
    pub payload_content_type: Option<PublicCloudKeyManagerPayloadContentType>,
    pub secret_type: PublicCloudKeyManagerSecretType,
}

/// `publicCloud.keyManager.Secret`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerSecret {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<KeyManagerSecretCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<KeyManagerSecretTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.keyManager.SecretConsumer`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerSecretConsumer {
    pub id: Option<String>,
    pub resource_id: Option<String>,
    pub resource_type: Option<PublicCloudKeyManagerConsumerResourceType>,
    pub service: Option<PublicCloudKeyManagerConsumerService>,
}

/// `publicCloud.keyManager.SecretConsumerInput`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerSecretConsumerInput {
    pub resource_id: String,
    pub resource_type: PublicCloudKeyManagerConsumerResourceType,
    pub service: PublicCloudKeyManagerConsumerService,
}

/// `publicCloud.keyManager.SecretCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerSecretCreation {
    pub target_spec: KeyManagerSecretTargetSpec,
}

/// `publicCloud.keyManager.SecretPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerSecretPayload {
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// `publicCloud.keyManager.SecretUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerSecretUpdateTargetSpec {
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// `publicCloud.keyManager.SecretUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyManagerSecretUpdate {
    pub checksum: String,
    pub target_spec: KeyManagerSecretUpdateTargetSpec,
}

/// `publicCloud.loadbalancer.HealthMonitorTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudLoadbalancerHealthMonitorType {
    #[serde(rename = "HTTP")]
    Http,
    #[serde(rename = "HTTPS")]
    Https,
    #[serde(rename = "PING")]
    Ping,
    #[serde(rename = "SCTP")]
    Sctp,
    #[serde(rename = "TCP")]
    Tcp,
    #[serde(rename = "TLS_HELLO")]
    TlsHello,
    #[serde(rename = "UDP_CONNECT")]
    UdpConnect,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.loadbalancer.HealthMonitorCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerHealthMonitorCurrentState {
    pub delay: Option<i64>,
    pub domain_name: Option<String>,
    pub expected_codes: Option<String>,
    pub http_method: Option<String>,
    pub http_version: Option<String>,
    pub id: Option<String>,
    pub max_retries: Option<i64>,
    pub max_retries_down: Option<i64>,
    pub name: Option<String>,
    pub operating_status: Option<String>,
    pub provisioning_status: Option<String>,
    pub timeout: Option<i64>,
    #[serde(rename = "type")]
    pub kind: Option<PublicCloudLoadbalancerHealthMonitorType>,
    pub url_path: Option<String>,
}

/// `publicCloud.loadbalancer.HealthMonitorHttpMethodEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudLoadbalancerHealthMonitorHttpMethod {
    #[serde(rename = "CONNECT")]
    Connect,
    #[serde(rename = "DELETE")]
    Delete,
    #[serde(rename = "GET")]
    Get,
    #[serde(rename = "HEAD")]
    Head,
    #[serde(rename = "OPTIONS")]
    Options,
    #[serde(rename = "PATCH")]
    Patch,
    #[serde(rename = "POST")]
    Post,
    #[serde(rename = "PUT")]
    Put,
    #[serde(rename = "TRACE")]
    Trace,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.loadbalancer.HealthMonitorTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerHealthMonitorTargetSpec {
    pub delay: i64,
    pub domain_name: Option<String>,
    pub expected_codes: Option<String>,
    pub http_method: Option<PublicCloudLoadbalancerHealthMonitorHttpMethod>,
    pub http_version: Option<String>,
    pub max_retries: i64,
    pub max_retries_down: Option<i64>,
    pub name: Option<String>,
    pub timeout: i64,
    #[serde(rename = "type")]
    pub kind: PublicCloudLoadbalancerHealthMonitorType,
    pub url_path: Option<String>,
}

/// `publicCloud.loadbalancer.HealthMonitorUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerHealthMonitorUpdateTargetSpec {
    pub delay: i64,
    pub domain_name: Option<String>,
    pub expected_codes: Option<String>,
    pub http_method: Option<PublicCloudLoadbalancerHealthMonitorHttpMethod>,
    pub http_version: Option<String>,
    pub max_retries: i64,
    pub max_retries_down: Option<i64>,
    pub name: Option<String>,
    pub timeout: i64,
    pub url_path: Option<String>,
}

/// `publicCloud.loadbalancer.L7PolicyActionEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudLoadbalancerL7PolicyAction {
    #[serde(rename = "REDIRECT_PREFIX")]
    RedirectPrefix,
    #[serde(rename = "REDIRECT_TO_POOL")]
    RedirectToPool,
    #[serde(rename = "REDIRECT_TO_URL")]
    RedirectToUrl,
    #[serde(rename = "REJECT")]
    Reject,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.loadbalancer.L7PolicyRedirectPoolDetail`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerL7PolicyRedirectPoolDetail {
    pub id: Option<String>,
}

/// `publicCloud.loadbalancer.LoadbalancerOperatingStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudLoadbalancerLoadbalancerOperatingStatus {
    #[serde(rename = "DEGRADED")]
    Degraded,
    #[serde(rename = "DRAINING")]
    Draining,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "NO_MONITOR")]
    NoMonitor,
    #[serde(rename = "OFFLINE")]
    Offline,
    #[serde(rename = "ONLINE")]
    Online,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.loadbalancer.LoadbalancerProvisioningStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudLoadbalancerLoadbalancerProvisioningStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "DELETED")]
    Deleted,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "PENDING_CREATE")]
    PendingCreate,
    #[serde(rename = "PENDING_DELETE")]
    PendingDelete,
    #[serde(rename = "PENDING_UPDATE")]
    PendingUpdate,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.loadbalancer.L7RuleState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerL7RuleState {
    pub compare_type: Option<String>,
    pub id: Option<String>,
    pub invert: Option<bool>,
    pub key: Option<String>,
    pub operating_status: Option<PublicCloudLoadbalancerLoadbalancerOperatingStatus>,
    pub provisioning_status: Option<PublicCloudLoadbalancerLoadbalancerProvisioningStatus>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub value: Option<String>,
}

/// `publicCloud.loadbalancer.L7PolicyCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerL7PolicyCurrentState {
    pub action: Option<PublicCloudLoadbalancerL7PolicyAction>,
    pub description: Option<String>,
    pub name: Option<String>,
    pub operating_status: Option<PublicCloudLoadbalancerLoadbalancerOperatingStatus>,
    pub position: Option<i64>,
    pub provisioning_status: Option<PublicCloudLoadbalancerLoadbalancerProvisioningStatus>,
    pub redirect_http_code: Option<i64>,
    pub redirect_pool: Option<LoadbalancerL7PolicyRedirectPoolDetail>,
    pub redirect_prefix: Option<String>,
    pub redirect_url: Option<String>,
    pub rules: Option<Vec<LoadbalancerL7RuleState>>,
}

/// `publicCloud.loadbalancer.L7PolicyRedirectPoolRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerL7PolicyRedirectPoolRef {
    pub id: String,
}

/// `publicCloud.loadbalancer.L7RuleCompareTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudLoadbalancerL7RuleCompareType {
    #[serde(rename = "CONTAINS")]
    Contains,
    #[serde(rename = "ENDS_WITH")]
    EndsWith,
    #[serde(rename = "EQUAL_TO")]
    EqualTo,
    #[serde(rename = "REGEX")]
    Regex,
    #[serde(rename = "STARTS_WITH")]
    StartsWith,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.loadbalancer.L7RuleTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudLoadbalancerL7RuleType {
    #[serde(rename = "COOKIE")]
    Cookie,
    #[serde(rename = "FILE_TYPE")]
    FileType,
    #[serde(rename = "HEADER")]
    Header,
    #[serde(rename = "HOST_NAME")]
    HostName,
    #[serde(rename = "PATH")]
    Path,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.loadbalancer.L7RuleSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerL7RuleSpec {
    pub compare_type: PublicCloudLoadbalancerL7RuleCompareType,
    pub invert: Option<bool>,
    pub key: Option<String>,
    #[serde(rename = "type")]
    pub kind: PublicCloudLoadbalancerL7RuleType,
    pub value: String,
}

/// `publicCloud.loadbalancer.L7PolicyTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerL7PolicyTargetSpec {
    pub action: PublicCloudLoadbalancerL7PolicyAction,
    pub description: Option<String>,
    pub name: Option<String>,
    pub position: Option<i64>,
    pub redirect_http_code: Option<i64>,
    pub redirect_pool: Option<LoadbalancerL7PolicyRedirectPoolRef>,
    pub redirect_prefix: Option<String>,
    pub redirect_url: Option<String>,
    pub rules: Option<Vec<LoadbalancerL7RuleSpec>>,
}

/// `publicCloud.loadbalancer.L7Policy`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerL7Policy {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<LoadbalancerL7PolicyCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<LoadbalancerL7PolicyTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.loadbalancer.L7PolicyCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerL7PolicyCreation {
    pub target_spec: LoadbalancerL7PolicyTargetSpec,
}

/// `publicCloud.loadbalancer.L7PolicyUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerL7PolicyUpdateTargetSpec {
    pub action: PublicCloudLoadbalancerL7PolicyAction,
    pub description: Option<String>,
    pub name: Option<String>,
    pub position: Option<i64>,
    pub redirect_http_code: Option<i64>,
    pub redirect_pool: Option<LoadbalancerL7PolicyRedirectPoolRef>,
    pub redirect_prefix: Option<String>,
    pub redirect_url: Option<String>,
    pub rules: Option<Vec<LoadbalancerL7RuleSpec>>,
}

/// `publicCloud.loadbalancer.L7PolicyUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerL7PolicyUpdate {
    pub checksum: String,
    pub target_spec: LoadbalancerL7PolicyUpdateTargetSpec,
}

/// `publicCloud.loadbalancer.ListenerDefaultPoolDetail`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerListenerDefaultPoolDetail {
    pub id: Option<String>,
}

/// `publicCloud.loadbalancer.ListenerInsertHeaders`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerListenerInsertHeaders {
    pub x_forwarded_for: Option<bool>,
    pub x_forwarded_port: Option<bool>,
    pub x_forwarded_proto: Option<bool>,
    pub x_ssl_client_dn: Option<bool>,
    pub x_ssl_client_has_cert: Option<bool>,
    pub x_ssl_client_verify: Option<bool>,
}

/// `publicCloud.loadbalancer.ListenerProtocolEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudLoadbalancerListenerProtocol {
    #[serde(rename = "HTTP")]
    Http,
    #[serde(rename = "HTTPS")]
    Https,
    #[serde(rename = "SCTP")]
    Sctp,
    #[serde(rename = "TCP")]
    Tcp,
    #[serde(rename = "TERMINATED_HTTPS")]
    TerminatedHttps,
    #[serde(rename = "UDP")]
    Udp,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.loadbalancer.ListenerCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerListenerCurrentState {
    pub allowed_cidrs: Option<Vec<String>>,
    pub connection_limit: Option<i64>,
    pub default_pool: Option<LoadbalancerListenerDefaultPoolDetail>,
    pub default_tls_container_ref: Option<String>,
    pub description: Option<String>,
    pub insert_headers: Option<LoadbalancerListenerInsertHeaders>,
    pub name: Option<String>,
    pub operating_status: Option<PublicCloudLoadbalancerLoadbalancerOperatingStatus>,
    pub protocol: Option<PublicCloudLoadbalancerListenerProtocol>,
    pub protocol_port: Option<i64>,
    pub provisioning_status: Option<PublicCloudLoadbalancerLoadbalancerProvisioningStatus>,
    pub sni_container_refs: Option<Vec<String>>,
    pub timeout_client_data: Option<i64>,
    pub timeout_member_connect: Option<i64>,
    pub timeout_member_data: Option<i64>,
    pub timeout_tcp_inspect: Option<i64>,
    pub tls_versions: Option<Vec<String>>,
}

/// `publicCloud.loadbalancer.ListenerDefaultPoolRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerListenerDefaultPoolRef {
    pub id: String,
}

/// `publicCloud.loadbalancer.ListenerTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerListenerTargetSpec {
    pub allowed_cidrs: Option<Vec<String>>,
    pub connection_limit: Option<i64>,
    pub default_pool: Option<LoadbalancerListenerDefaultPoolRef>,
    pub default_tls_container_ref: Option<String>,
    pub description: Option<String>,
    pub insert_headers: Option<LoadbalancerListenerInsertHeaders>,
    pub name: String,
    pub protocol: PublicCloudLoadbalancerListenerProtocol,
    pub protocol_port: i64,
    pub sni_container_refs: Option<Vec<String>>,
    pub timeout_client_data: Option<i64>,
    pub timeout_member_connect: Option<i64>,
    pub timeout_member_data: Option<i64>,
    pub timeout_tcp_inspect: Option<i64>,
    pub tls_versions: Option<Vec<String>>,
}

/// `publicCloud.loadbalancer.Listener`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerListener {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<LoadbalancerListenerCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<LoadbalancerListenerTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.loadbalancer.ListenerCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerListenerCreation {
    pub target_spec: LoadbalancerListenerTargetSpec,
}

/// `publicCloud.loadbalancer.ListenerPutTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerListenerPutTargetSpec {
    pub allowed_cidrs: Option<Vec<String>>,
    pub connection_limit: Option<i64>,
    pub default_pool: Option<LoadbalancerListenerDefaultPoolRef>,
    pub default_tls_container_ref: Option<String>,
    pub description: Option<String>,
    pub insert_headers: Option<LoadbalancerListenerInsertHeaders>,
    pub name: String,
    pub sni_container_refs: Option<Vec<String>>,
    pub timeout_client_data: Option<i64>,
    pub timeout_member_connect: Option<i64>,
    pub timeout_member_data: Option<i64>,
    pub timeout_tcp_inspect: Option<i64>,
    pub tls_versions: Option<Vec<String>>,
}

/// `publicCloud.loadbalancer.ListenerUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerListenerUpdate {
    pub checksum: String,
    pub target_spec: LoadbalancerListenerPutTargetSpec,
}

/// `publicCloud.loadbalancer.LoadbalancerFlavorNameEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudLoadbalancerLoadbalancerFlavorName {
    #[serde(rename = "LARGE")]
    Large,
    #[serde(rename = "MEDIUM")]
    Medium,
    #[serde(rename = "SMALL")]
    Small,
    #[serde(rename = "XL")]
    Xl,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.loadbalancer.LoadbalancerFlavorRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerLoadbalancerFlavorRef {
    pub name: PublicCloudLoadbalancerLoadbalancerFlavorName,
}

/// `publicCloud.loadbalancer.LoadbalancerNetworkRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerLoadbalancerNetworkRef {
    pub id: String,
}

/// `publicCloud.loadbalancer.LoadbalancerSubnetRef`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerLoadbalancerSubnetRef {
    pub id: String,
}

/// `publicCloud.loadbalancer.LoadbalancerCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerLoadbalancerCurrentState {
    pub description: Option<String>,
    pub flavor: Option<LoadbalancerLoadbalancerFlavorRef>,
    pub location: Option<CommonLocation>,
    pub name: Option<String>,
    pub operating_status: Option<PublicCloudLoadbalancerLoadbalancerOperatingStatus>,
    pub provisioning_status: Option<PublicCloudLoadbalancerLoadbalancerProvisioningStatus>,
    #[serde(default)]
    pub vip_address: serde_json::Value,
    pub vip_network: Option<LoadbalancerLoadbalancerNetworkRef>,
    pub vip_subnet: Option<LoadbalancerLoadbalancerSubnetRef>,
}

/// `publicCloud.loadbalancer.LoadbalancerTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerLoadbalancerTargetSpec {
    pub description: Option<String>,
    pub flavor: LoadbalancerLoadbalancerFlavorRef,
    pub location: CommonLocation,
    pub name: String,
    pub vip_network: LoadbalancerLoadbalancerNetworkRef,
    pub vip_subnet: LoadbalancerLoadbalancerSubnetRef,
}

/// `publicCloud.loadbalancer.Loadbalancer`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerLoadbalancer {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<LoadbalancerLoadbalancerCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<LoadbalancerLoadbalancerTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.loadbalancer.LoadbalancerCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerLoadbalancerCreation {
    pub target_spec: LoadbalancerLoadbalancerTargetSpec,
}

/// `publicCloud.loadbalancer.LoadbalancerPutTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerLoadbalancerPutTargetSpec {
    pub description: Option<String>,
    pub name: String,
}

/// `publicCloud.loadbalancer.LoadbalancerUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerLoadbalancerUpdate {
    pub checksum: String,
    pub target_spec: LoadbalancerLoadbalancerPutTargetSpec,
}

/// `publicCloud.loadbalancer.MemberMonitor`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerMemberMonitor {
    #[serde(default)]
    pub address: serde_json::Value,
    pub port: Option<i64>,
}

/// `publicCloud.loadbalancer.MemberCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerMemberCurrentState {
    #[serde(default)]
    pub address: serde_json::Value,
    pub backup: Option<bool>,
    pub monitor: Option<LoadbalancerMemberMonitor>,
    pub name: Option<String>,
    pub operating_status: Option<PublicCloudLoadbalancerLoadbalancerOperatingStatus>,
    pub protocol_port: Option<i64>,
    pub provisioning_status: Option<PublicCloudLoadbalancerLoadbalancerProvisioningStatus>,
    pub subnet: Option<LoadbalancerLoadbalancerSubnetRef>,
    pub weight: Option<i64>,
}

/// `publicCloud.loadbalancer.MemberTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerMemberTargetSpec {
    #[serde(default)]
    pub address: serde_json::Value,
    pub backup: Option<bool>,
    pub monitor: Option<LoadbalancerMemberMonitor>,
    pub name: Option<String>,
    pub protocol_port: i64,
    pub subnet: Option<LoadbalancerLoadbalancerSubnetRef>,
    pub weight: Option<i64>,
}

/// `publicCloud.loadbalancer.Member`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerMember {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<LoadbalancerMemberCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<LoadbalancerMemberTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.loadbalancer.MemberCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerMemberCreation {
    pub target_spec: LoadbalancerMemberTargetSpec,
}

/// `publicCloud.loadbalancer.MemberUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerMemberUpdateTargetSpec {
    pub backup: Option<bool>,
    pub monitor: Option<LoadbalancerMemberMonitor>,
    pub name: Option<String>,
    pub weight: Option<i64>,
}

/// `publicCloud.loadbalancer.MemberUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerMemberUpdate {
    pub checksum: String,
    pub target_spec: LoadbalancerMemberUpdateTargetSpec,
}

/// `publicCloud.loadbalancer.PoolAlgorithmEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudLoadbalancerPoolAlgorithm {
    #[serde(rename = "LEAST_CONNECTIONS")]
    LeastConnections,
    #[serde(rename = "ROUND_ROBIN")]
    RoundRobin,
    #[serde(rename = "SOURCE_IP")]
    SourceIp,
    #[serde(rename = "SOURCE_IP_PORT")]
    SourceIpPort,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.loadbalancer.PoolProtocolEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudLoadbalancerPoolProtocol {
    #[serde(rename = "HTTP")]
    Http,
    #[serde(rename = "HTTPS")]
    Https,
    #[serde(rename = "PROXY")]
    Proxy,
    #[serde(rename = "PROXYV2")]
    Proxyv2,
    #[serde(rename = "SCTP")]
    Sctp,
    #[serde(rename = "TCP")]
    Tcp,
    #[serde(rename = "UDP")]
    Udp,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.loadbalancer.SessionPersistenceTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudLoadbalancerSessionPersistenceType {
    #[serde(rename = "APP_COOKIE")]
    AppCookie,
    #[serde(rename = "HTTP_COOKIE")]
    HttpCookie,
    #[serde(rename = "SOURCE_IP")]
    SourceIp,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.loadbalancer.SessionPersistence`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerSessionPersistence {
    pub cookie_name: Option<String>,
    #[serde(rename = "type")]
    pub kind: PublicCloudLoadbalancerSessionPersistenceType,
}

/// `publicCloud.loadbalancer.PoolCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerPoolCurrentState {
    pub algorithm: Option<PublicCloudLoadbalancerPoolAlgorithm>,
    pub description: Option<String>,
    pub health_monitor: Option<LoadbalancerHealthMonitorCurrentState>,
    pub name: Option<String>,
    pub operating_status: Option<PublicCloudLoadbalancerLoadbalancerOperatingStatus>,
    pub persistence: Option<LoadbalancerSessionPersistence>,
    pub protocol: Option<PublicCloudLoadbalancerPoolProtocol>,
    pub provisioning_status: Option<PublicCloudLoadbalancerLoadbalancerProvisioningStatus>,
}

/// `publicCloud.loadbalancer.PoolTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerPoolTargetSpec {
    pub algorithm: PublicCloudLoadbalancerPoolAlgorithm,
    pub description: Option<String>,
    pub health_monitor: Option<LoadbalancerHealthMonitorTargetSpec>,
    pub name: Option<String>,
    pub persistence: Option<LoadbalancerSessionPersistence>,
    pub protocol: PublicCloudLoadbalancerPoolProtocol,
}

/// `publicCloud.loadbalancer.Pool`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerPool {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<LoadbalancerPoolCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<LoadbalancerPoolTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.loadbalancer.PoolCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerPoolCreation {
    pub target_spec: LoadbalancerPoolTargetSpec,
}

/// `publicCloud.loadbalancer.PoolUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerPoolUpdateTargetSpec {
    pub algorithm: PublicCloudLoadbalancerPoolAlgorithm,
    pub description: Option<String>,
    pub health_monitor: Option<LoadbalancerHealthMonitorUpdateTargetSpec>,
    pub name: Option<String>,
    pub persistence: Option<LoadbalancerSessionPersistence>,
}

/// `publicCloud.loadbalancer.PoolUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadbalancerPoolUpdate {
    pub checksum: String,
    pub target_spec: LoadbalancerPoolUpdateTargetSpec,
}

/// `publicCloud.network.NetworkCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkNetworkCurrentState {
    pub description: Option<String>,
    pub location: Option<CommonLocation>,
    pub name: Option<String>,
}

/// `publicCloud.network.NetworkTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkNetworkTargetSpec {
    pub description: Option<String>,
    pub location: CommonLocation,
    pub name: String,
}

/// `publicCloud.network.Network`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkNetwork {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<NetworkNetworkCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<NetworkNetworkTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.network.NetworkCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkNetworkCreation {
    pub target_spec: NetworkNetworkTargetSpec,
}

/// `publicCloud.network.NetworkPutTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkNetworkPutTargetSpec {
    pub name: String,
}

/// `publicCloud.network.NetworkUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkNetworkUpdate {
    pub checksum: String,
    pub target_spec: NetworkNetworkPutTargetSpec,
}

/// `publicCloud.network.SubnetAllocationPool`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSubnetAllocationPool {
    #[serde(default)]
    pub end: serde_json::Value,
    #[serde(default)]
    pub start: serde_json::Value,
}

/// `publicCloud.network.SubnetHostRoute`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSubnetHostRoute {
    pub destination: Option<String>,
    #[serde(default)]
    pub next_hop: serde_json::Value,
}

/// `publicCloud.network.SubnetCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSubnetCurrentState {
    pub allocation_pools: Option<Vec<NetworkSubnetAllocationPool>>,
    pub cidr: Option<String>,
    pub description: Option<String>,
    pub dhcp_enabled: Option<bool>,
    pub dns_nameservers: Option<Vec<String>>,
    #[serde(default)]
    pub gateway_ip: serde_json::Value,
    pub host_routes: Option<Vec<NetworkSubnetHostRoute>>,
    pub location: Option<CommonLocation>,
    pub name: Option<String>,
}

/// `publicCloud.network.SubnetTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSubnetTargetSpec {
    pub allocation_pools: Option<Vec<NetworkSubnetAllocationPool>>,
    pub cidr: String,
    pub description: Option<String>,
    pub dhcp_enabled: Option<bool>,
    pub dns_nameservers: Option<Vec<String>>,
    #[serde(default)]
    pub gateway_ip: serde_json::Value,
    pub location: CommonLocation,
    pub name: String,
}

/// `publicCloud.network.Subnet`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSubnet {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<NetworkSubnetCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<NetworkSubnetTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.network.SubnetCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSubnetCreation {
    pub target_spec: NetworkSubnetTargetSpec,
}

/// `publicCloud.network.SubnetPutTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSubnetPutTargetSpec {
    pub allocation_pools: Option<Vec<NetworkSubnetAllocationPool>>,
    pub description: Option<String>,
    pub dhcp_enabled: Option<bool>,
    pub dns_nameservers: Option<Vec<String>>,
    #[serde(default)]
    pub gateway_ip: serde_json::Value,
    pub name: String,
}

/// `publicCloud.network.SubnetUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSubnetUpdate {
    pub checksum: String,
    pub target_spec: NetworkSubnetPutTargetSpec,
}

/// `publicCloud.project.ModeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudProjectMode {
    #[serde(rename = "CLASSIC")]
    Classic,
    #[serde(rename = "DISCOVERY")]
    Discovery,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.project.ProjectCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProjectCurrentState {
    pub mode: Option<PublicCloudProjectMode>,
    pub name: Option<String>,
}

/// `publicCloud.project.ProjectTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProjectTargetSpec {
    pub mode: PublicCloudProjectMode,
    pub name: String,
}

/// `publicCloud.project.ProjectAsync`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProjectAsync {
    pub created_at: Option<String>,
    pub current_state: Option<ProjectProjectCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<ProjectProjectTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.project.ProjectAsyncWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProjectAsyncWithIAM {
    pub created_at: Option<String>,
    pub current_state: Option<ProjectProjectCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<ProjectProjectTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.project.ProjectCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProjectCreation {
    pub target_spec: ProjectProjectTargetSpec,
}

/// `publicCloud.quota.QuotaProfileCompute`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaProfileCompute {
    pub cores: Option<i64>,
    pub instances: Option<i64>,
    pub memory: Option<i64>,
}

/// `publicCloud.quota.QuotaProfileKeyManager`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaProfileKeyManager {
    pub containers: Option<i64>,
    pub secrets: Option<i64>,
}

/// `publicCloud.quota.QuotaProfileKeypair`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaProfileKeypair {
    pub keypairs: Option<i64>,
}

/// `publicCloud.quota.QuotaProfileLoadbalancer`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaProfileLoadbalancer {
    pub health_monitors: Option<i64>,
    pub l7_policies: Option<i64>,
    pub l7_rules: Option<i64>,
    pub listeners: Option<i64>,
    pub loadbalancers: Option<i64>,
    pub members: Option<i64>,
    pub pools: Option<i64>,
}

/// `publicCloud.quota.QuotaProfileNetwork`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaProfileNetwork {
    pub floating_ips: Option<i64>,
    pub gateways: Option<i64>,
    pub networks: Option<i64>,
    pub security_group_rules: Option<i64>,
    pub security_groups: Option<i64>,
    pub subnets: Option<i64>,
}

/// `publicCloud.quota.QuotaProfileShare`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaProfileShare {
    pub backup_size_total: Option<i64>,
    pub backups: Option<i64>,
    pub shares: Option<i64>,
    pub size_total: Option<i64>,
    pub snapshots: Option<i64>,
}

/// `publicCloud.quota.QuotaProfileVolume`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaProfileVolume {
    pub backup_size_total: Option<i64>,
    pub backups: Option<i64>,
    pub size_total: Option<i64>,
    pub snapshots: Option<i64>,
    pub volumes: Option<i64>,
}

/// `publicCloud.quota.QuotaProfile`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaProfile {
    pub compute: Option<QuotaQuotaProfileCompute>,
    pub key_manager: Option<QuotaQuotaProfileKeyManager>,
    pub keypair: Option<QuotaQuotaProfileKeypair>,
    pub loadbalancer: Option<QuotaQuotaProfileLoadbalancer>,
    pub name: Option<String>,
    pub network: Option<QuotaQuotaProfileNetwork>,
    pub share: Option<QuotaQuotaProfileShare>,
    pub volume: Option<QuotaQuotaProfileVolume>,
}

/// `publicCloud.quota.QuotaUnitEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudQuotaQuotaUnit {
    #[serde(rename = "COUNT")]
    Count,
    #[serde(rename = "GB")]
    Gb,
    #[serde(rename = "MB")]
    Mb,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.quota.QuotaUsage`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaUsage {
    pub limit: Option<i64>,
    pub unit: Option<PublicCloudQuotaQuotaUnit>,
    pub used: Option<i64>,
}

/// `publicCloud.quota.QuotaRegionCompute`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaRegionCompute {
    pub cores: Option<QuotaQuotaUsage>,
    pub instances: Option<QuotaQuotaUsage>,
    pub memory: Option<QuotaQuotaUsage>,
}

/// `publicCloud.quota.QuotaRegionKeyManager`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaRegionKeyManager {
    pub containers: Option<QuotaQuotaUsage>,
    pub secrets: Option<QuotaQuotaUsage>,
}

/// `publicCloud.quota.QuotaRegionKeypair`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaRegionKeypair {
    pub keypairs: Option<QuotaQuotaUsage>,
}

/// `publicCloud.quota.QuotaRegionLoadbalancer`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaRegionLoadbalancer {
    pub health_monitors: Option<QuotaQuotaUsage>,
    pub l7_policies: Option<QuotaQuotaUsage>,
    pub l7_rules: Option<QuotaQuotaUsage>,
    pub listeners: Option<QuotaQuotaUsage>,
    pub loadbalancers: Option<QuotaQuotaUsage>,
    pub members: Option<QuotaQuotaUsage>,
    pub pools: Option<QuotaQuotaUsage>,
}

/// `publicCloud.quota.QuotaRegionNetwork`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaRegionNetwork {
    pub floating_ips: Option<QuotaQuotaUsage>,
    pub gateways: Option<QuotaQuotaUsage>,
    pub networks: Option<QuotaQuotaUsage>,
    pub security_group_rules: Option<QuotaQuotaUsage>,
    pub security_groups: Option<QuotaQuotaUsage>,
    pub subnets: Option<QuotaQuotaUsage>,
}

/// `publicCloud.quota.QuotaLimit`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaLimit {
    pub limit: Option<i64>,
    pub unit: Option<PublicCloudQuotaQuotaUnit>,
}

/// `publicCloud.quota.QuotaRegionShare`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaRegionShare {
    pub backup_size_total: Option<QuotaQuotaUsage>,
    pub backups: Option<QuotaQuotaUsage>,
    pub per_share_size: Option<QuotaQuotaLimit>,
    pub share_networks: Option<QuotaQuotaUsage>,
    pub shares: Option<QuotaQuotaUsage>,
    pub size_total: Option<QuotaQuotaUsage>,
    pub snapshot_size_total: Option<QuotaQuotaUsage>,
    pub snapshots: Option<QuotaQuotaUsage>,
}

/// `publicCloud.quota.QuotaRegionVolume`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaRegionVolume {
    pub backup_size_total: Option<QuotaQuotaUsage>,
    pub backups: Option<QuotaQuotaUsage>,
    pub per_volume_size: Option<QuotaQuotaLimit>,
    pub size_total: Option<QuotaQuotaUsage>,
    pub snapshots: Option<QuotaQuotaUsage>,
    pub volumes: Option<QuotaQuotaUsage>,
}

/// `publicCloud.quota.QuotaUsageDetails`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaUsageDetails {
    pub compute: Option<QuotaQuotaRegionCompute>,
    pub key_manager: Option<QuotaQuotaRegionKeyManager>,
    pub keypair: Option<QuotaQuotaRegionKeypair>,
    pub loadbalancer: Option<QuotaQuotaRegionLoadbalancer>,
    pub network: Option<QuotaQuotaRegionNetwork>,
    pub share: Option<QuotaQuotaRegionShare>,
    pub volume: Option<QuotaQuotaRegionVolume>,
}

/// `publicCloud.quota.QuotaRegionCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaRegionCurrentState {
    pub location: Option<CommonLocation>,
    pub profile: Option<String>,
    pub usage: Option<QuotaQuotaUsageDetails>,
}

/// `publicCloud.quota.QuotaCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaCurrentState {
    pub available_profiles: Option<Vec<QuotaQuotaProfile>>,
    pub prevent_automatic_quota_upgrade: Option<bool>,
    pub regions: Option<Vec<QuotaQuotaRegionCurrentState>>,
}

/// `publicCloud.quota.QuotaRegionTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaRegionTargetSpec {
    pub location: CommonLocation,
    pub profile: String,
}

/// `publicCloud.quota.QuotaTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaTargetSpec {
    pub prevent_automatic_quota_upgrade: Option<bool>,
    pub regions: Option<Vec<QuotaQuotaRegionTargetSpec>>,
}

/// `publicCloud.quota.Quota`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuota {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<QuotaQuotaCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<QuotaQuotaTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.quota.QuotaUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaUpdateTargetSpec {
    pub prevent_automatic_quota_upgrade: bool,
    #[serde(default)]
    pub regions: Vec<QuotaQuotaRegionTargetSpec>,
}

/// `publicCloud.quota.QuotaUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaQuotaUpdate {
    pub checksum: String,
    pub target_spec: QuotaQuotaUpdateTargetSpec,
}

/// `publicCloud.rancher.Credentials`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherCredentials {
    #[serde(default)]
    pub password: serde_json::Value,
    pub username: Option<String>,
}

/// `publicCloud.rancher.EligibilityReference`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherEligibilityReference {
    pub free_trial: Option<bool>,
}

/// `publicCloud.rancher.IpRestriction`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherIpRestriction {
    pub cidr_block: String,
    pub description: String,
}

/// `publicCloud.rancher.Networking`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherNetworking {
    pub egress_cidr_blocks: Option<Vec<String>>,
}

/// `publicCloud.rancher.PlanCapabilityStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudRancherPlanCapabilityStatus {
    #[serde(rename = "AVAILABLE")]
    Available,
    #[serde(rename = "CURRENT")]
    Current,
    #[serde(rename = "UNAVAILABLE")]
    Unavailable,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.rancher.PlanEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudRancherPlan {
    #[serde(rename = "OVHCLOUD_EDITION")]
    OvhcloudEdition,
    #[serde(rename = "STANDARD")]
    Standard,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.rancher.PlanUnavailabilityCauseEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudRancherPlanUnavailabilityCause {
    #[serde(rename = "CANNOT_DOWNGRADE_USING_HIGHER_FEATURES")]
    CannotDowngradeUsingHigherFeatures,
    #[serde(rename = "CANNOT_SWITCH_PLAN_FOR_ALPHA")]
    CannotSwitchPlanForAlpha,
    #[serde(rename = "NOT_IMPLEMENTED")]
    NotImplemented,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.rancher.PlanCapability`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherPlanCapability {
    pub cause: Option<PublicCloudRancherPlanUnavailabilityCause>,
    pub message: Option<String>,
    pub name: Option<PublicCloudRancherPlan>,
    pub status: Option<PublicCloudRancherPlanCapabilityStatus>,
}

/// `publicCloud.rancher.PlanReferenceStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudRancherPlanReferenceStatus {
    #[serde(rename = "AVAILABLE")]
    Available,
    #[serde(rename = "UNAVAILABLE")]
    Unavailable,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.rancher.PlanReference`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherPlanReference {
    pub cause: Option<PublicCloudRancherPlanUnavailabilityCause>,
    pub message: Option<String>,
    pub name: Option<PublicCloudRancherPlan>,
    pub status: Option<PublicCloudRancherPlanReferenceStatus>,
}

/// `publicCloud.rancher.RegionEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudRancherRegion {
    #[serde(rename = "EU_WEST_GRA")]
    EuWestGra,
    #[serde(rename = "EU_WEST_RBX")]
    EuWestRbx,
    #[serde(rename = "EU_WEST_SBG")]
    EuWestSbg,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.rancher.Usage`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherUsage {
    pub datetime: Option<String>,
    pub orchestrated_vcpus: Option<i64>,
}

/// `publicCloud.rancher.RancherCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherRancherCurrentState {
    #[serde(default)]
    pub bootstrap_password: serde_json::Value,
    pub iam_auth_enabled: Option<bool>,
    pub ip_restrictions: Option<Vec<RancherIpRestriction>>,
    pub name: Option<String>,
    pub networking: Option<RancherNetworking>,
    pub plan: Option<PublicCloudRancherPlan>,
    pub region: Option<PublicCloudRancherRegion>,
    pub url: Option<String>,
    pub usage: Option<RancherUsage>,
    pub version: Option<String>,
}

/// `publicCloud.rancher.RancherTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherRancherTargetSpec {
    pub iam_auth_enabled: Option<bool>,
    pub ip_restrictions: Option<Vec<RancherIpRestriction>>,
    pub name: String,
    pub plan: PublicCloudRancherPlan,
    pub version: String,
}

/// `publicCloud.rancher.Rancher`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherRancher {
    pub created_at: Option<String>,
    pub current_state: Option<RancherRancherCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<RancherRancherTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.rancher.RancherCreationTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherRancherCreationTargetSpec {
    pub iam_auth_enabled: Option<bool>,
    pub name: String,
    pub plan: PublicCloudRancherPlan,
    pub version: Option<String>,
}

/// `publicCloud.rancher.RancherCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherRancherCreation {
    pub target_spec: RancherRancherCreationTargetSpec,
}

/// `publicCloud.rancher.RancherUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherRancherUpdate {
    pub target_spec: RancherRancherTargetSpec,
}

/// `publicCloud.rancher.VersionCapabilityStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudRancherVersionCapabilityStatus {
    #[serde(rename = "AVAILABLE")]
    Available,
    #[serde(rename = "UNAVAILABLE")]
    Unavailable,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.rancher.VersionUnavailabilityCauseEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudRancherVersionUnavailabilityCause {
    #[serde(rename = "CANNOT_UPGRADE_MULTIPLE_VERSIONS")]
    CannotUpgradeMultipleVersions,
    #[serde(rename = "DEPRECATED")]
    Deprecated,
    #[serde(rename = "DISABLED")]
    Disabled,
    #[serde(rename = "END_OF_LIFE")]
    EndOfLife,
    #[serde(rename = "END_OF_SALE")]
    EndOfSale,
    #[serde(rename = "END_OF_SUPPORT")]
    EndOfSupport,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.rancher.VersionCapability`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherVersionCapability {
    pub cause: Option<PublicCloudRancherVersionUnavailabilityCause>,
    pub changelog_url: Option<String>,
    pub message: Option<String>,
    pub name: Option<String>,
    pub status: Option<PublicCloudRancherVersionCapabilityStatus>,
}

/// `publicCloud.rancher.VersionReferenceStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudRancherVersionReferenceStatus {
    #[serde(rename = "AVAILABLE")]
    Available,
    #[serde(rename = "UNAVAILABLE")]
    Unavailable,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.rancher.VersionReference`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RancherVersionReference {
    pub cause: Option<PublicCloudRancherVersionUnavailabilityCause>,
    pub changelog_url: Option<String>,
    pub message: Option<String>,
    pub name: Option<String>,
    pub status: Option<PublicCloudRancherVersionReferenceStatus>,
}

/// `publicCloud.reference.RegionStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudReferenceRegionStatus {
    #[serde(rename = "DISABLED")]
    Disabled,
    #[serde(rename = "ENABLED")]
    Enabled,
    #[serde(rename = "MAINTENANCE")]
    Maintenance,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.reference.Region`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceRegion {
    pub availability_zones: Option<Vec<String>>,
    pub continent: Option<String>,
    pub country: Option<String>,
    pub datacenter_name: Option<String>,
    pub name: Option<String>,
    pub services: Option<Vec<String>>,
    pub status: Option<PublicCloudReferenceRegionStatus>,
}

/// `publicCloud.reference.instance.Flavor`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceInstanceFlavor {
    pub description: Option<String>,
    pub disk: Option<i64>,
    pub ephemeral: Option<i64>,
    pub id: Option<String>,
    pub is_public: Option<bool>,
    pub location: Option<CommonLocation>,
    pub name: Option<String>,
    pub ram: Option<i64>,
    pub swap: Option<i64>,
    pub vcpus: Option<i64>,
}

/// `publicCloud.reference.instance.Image`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceInstanceImage {
    pub created_at: Option<String>,
    pub id: Option<String>,
    pub location: Option<CommonLocation>,
    pub min_disk: Option<i64>,
    pub min_ram: Option<i64>,
    pub name: Option<String>,
    pub size: Option<i64>,
    pub status: Option<PublicCloudInstanceImageStatus>,
    pub updated_at: Option<String>,
    pub visibility: Option<PublicCloudInstanceImageVisibility>,
}

/// `publicCloud.securityGroup.EthernetTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudSecurityGroupEthernetType {
    #[serde(rename = "IPV4")]
    Ipv4,
    #[serde(rename = "IPV6")]
    Ipv6,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.securityGroup.ProtocolEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudSecurityGroupProtocol {
    #[serde(rename = "AH")]
    Ah,
    #[serde(rename = "DCCP")]
    Dccp,
    #[serde(rename = "EGP")]
    Egp,
    #[serde(rename = "ESP")]
    Esp,
    #[serde(rename = "GRE")]
    Gre,
    #[serde(rename = "ICMP")]
    Icmp,
    #[serde(rename = "ICMPV6")]
    Icmpv6,
    #[serde(rename = "IGMP")]
    Igmp,
    #[serde(rename = "OSPF")]
    Ospf,
    #[serde(rename = "PGM")]
    Pgm,
    #[serde(rename = "RSVP")]
    Rsvp,
    #[serde(rename = "SCTP")]
    Sctp,
    #[serde(rename = "TCP")]
    Tcp,
    #[serde(rename = "UDP")]
    Udp,
    #[serde(rename = "UDPLITE")]
    Udplite,
    #[serde(rename = "VRRP")]
    Vrrp,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.securityGroup.TrafficFlowEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudSecurityGroupTrafficFlow {
    #[serde(rename = "EGRESS")]
    Egress,
    #[serde(rename = "INGRESS")]
    Ingress,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.securityGroup.SecurityGroupStateRule`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupSecurityGroupStateRule {
    pub description: Option<String>,
    pub direction: Option<PublicCloudSecurityGroupTrafficFlow>,
    pub ethernet_type: Option<PublicCloudSecurityGroupEthernetType>,
    pub id: Option<String>,
    pub port_range_max: Option<i64>,
    pub port_range_min: Option<i64>,
    pub protocol: Option<PublicCloudSecurityGroupProtocol>,
    pub remote_group: Option<SecurityGroupSecurityGroupRef>,
    pub remote_ip_prefix: Option<String>,
}

/// `publicCloud.securityGroup.SecurityGroupCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupSecurityGroupCurrentState {
    pub default_rules: Option<Vec<SecurityGroupSecurityGroupStateRule>>,
    pub description: Option<String>,
    pub location: Option<CommonLocation>,
    pub name: Option<String>,
    pub rules: Option<Vec<SecurityGroupSecurityGroupStateRule>>,
}

/// `publicCloud.securityGroup.SecurityGroupTargetRule`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupSecurityGroupTargetRule {
    pub description: Option<String>,
    pub direction: PublicCloudSecurityGroupTrafficFlow,
    pub ethernet_type: PublicCloudSecurityGroupEthernetType,
    pub port_range_max: Option<i64>,
    pub port_range_min: Option<i64>,
    pub protocol: Option<PublicCloudSecurityGroupProtocol>,
    pub remote_group: Option<SecurityGroupSecurityGroupRef>,
    pub remote_ip_prefix: Option<String>,
}

/// `publicCloud.securityGroup.SecurityGroupTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupSecurityGroupTargetSpec {
    pub description: Option<String>,
    pub location: CommonLocation,
    pub name: String,
    pub rules: Option<Vec<SecurityGroupSecurityGroupTargetRule>>,
}

/// `publicCloud.securityGroup.SecurityGroup`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupSecurityGroup {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<SecurityGroupSecurityGroupCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<SecurityGroupSecurityGroupTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.securityGroup.SecurityGroupCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupSecurityGroupCreation {
    pub target_spec: SecurityGroupSecurityGroupTargetSpec,
}

/// `publicCloud.securityGroup.SecurityGroupUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupSecurityGroupUpdateTargetSpec {
    pub description: Option<String>,
    pub name: String,
    pub rules: Option<Vec<SecurityGroupSecurityGroupTargetRule>>,
}

/// `publicCloud.securityGroup.SecurityGroupUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityGroupSecurityGroupUpdate {
    pub checksum: String,
    pub target_spec: SecurityGroupSecurityGroupUpdateTargetSpec,
}

/// `publicCloud.sshKey.SSHKey`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeySSHKey {
    pub created_at: Option<String>,
    pub name: Option<String>,
    pub public_key: Option<String>,
    pub updated_at: Option<String>,
}

/// `publicCloud.sshKey.SSHKeyCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeySSHKeyCreation {
    pub name: String,
    pub public_key: String,
}

/// `publicCloud.storage.file.FileStorageAccessLevelEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudStorageFileFileStorageAccessLevel {
    #[serde(rename = "READ_ONLY")]
    ReadOnly,
    #[serde(rename = "READ_WRITE")]
    ReadWrite,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.storage.file.FileStorageAccessRuleStateEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudStorageFileFileStorageAccessRuleState {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "APPLYING")]
    Applying,
    #[serde(rename = "DENYING")]
    Denying,
    #[serde(rename = "ERROR")]
    Error,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.storage.file.FileStorageAccessRule`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageAccessRule {
    pub access_level: Option<PublicCloudStorageFileFileStorageAccessLevel>,
    pub access_to: Option<String>,
    pub created_at: Option<String>,
    pub id: Option<String>,
    pub state: Option<PublicCloudStorageFileFileStorageAccessRuleState>,
}

/// `publicCloud.storage.file.FileStorageExportLocation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageExportLocation {
    pub path: Option<String>,
    pub preferred: Option<bool>,
}

/// `publicCloud.storage.file.FileStorageProtocolEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudStorageFileFileStorageProtocol {
    #[serde(rename = "NFS")]
    Nfs,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.storage.file.FileStorageTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudStorageFileFileStorageType {
    #[serde(rename = "STANDARD_1AZ")]
    Standard1Az,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.storage.file.FileStorageCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageCurrentState {
    pub access_rules: Option<Vec<StorageFileFileStorageAccessRule>>,
    pub description: Option<String>,
    pub export_locations: Option<Vec<StorageFileFileStorageExportLocation>>,
    pub location: Option<CommonLocation>,
    pub name: Option<String>,
    pub network_id: Option<String>,
    pub protocol: Option<PublicCloudStorageFileFileStorageProtocol>,
    pub share_type: Option<PublicCloudStorageFileFileStorageType>,
    pub size: Option<i64>,
    pub subnet_id: Option<String>,
}

/// `publicCloud.storage.file.FileStorageAccessRuleInput`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageAccessRuleInput {
    pub access_level: PublicCloudStorageFileFileStorageAccessLevel,
    pub access_to: String,
}

/// `publicCloud.storage.file.FileStorageTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageTargetSpec {
    pub access_rules: Option<Vec<StorageFileFileStorageAccessRuleInput>>,
    pub description: Option<String>,
    pub location: CommonLocation,
    pub name: String,
    pub network_id: String,
    pub protocol: PublicCloudStorageFileFileStorageProtocol,
    pub share_type: PublicCloudStorageFileFileStorageType,
    pub size: i64,
    pub subnet_id: String,
}

/// `publicCloud.storage.file.FileStorage`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorage {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<StorageFileFileStorageCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<StorageFileFileStorageTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.storage.file.FileStorageCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageCreation {
    pub target_spec: StorageFileFileStorageTargetSpec,
}

/// `publicCloud.storage.file.FileStorageSnapshotCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageSnapshotCurrentState {
    pub description: Option<String>,
    pub location: Option<CommonLocation>,
    pub name: Option<String>,
    pub share_id: Option<String>,
    pub share_proto: Option<PublicCloudStorageFileFileStorageProtocol>,
    pub share_size: Option<i64>,
    pub snapshot_size: Option<i64>,
}

/// `publicCloud.storage.file.FileStorageSnapshotTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageSnapshotTargetSpec {
    pub description: Option<String>,
    pub location: CommonLocation,
    pub name: Option<String>,
    pub share_id: String,
}

/// `publicCloud.storage.file.FileStorageSnapshot`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageSnapshot {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<StorageFileFileStorageSnapshotCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<StorageFileFileStorageSnapshotTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.storage.file.FileStorageSnapshotCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageSnapshotCreation {
    pub target_spec: StorageFileFileStorageSnapshotTargetSpec,
}

/// `publicCloud.storage.file.FileStorageSnapshotUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageSnapshotUpdateTargetSpec {
    pub description: Option<String>,
    pub name: Option<String>,
}

/// `publicCloud.storage.file.FileStorageSnapshotUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageSnapshotUpdate {
    pub checksum: String,
    pub target_spec: StorageFileFileStorageSnapshotUpdateTargetSpec,
}

/// `publicCloud.storage.file.FileStorageUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageUpdateTargetSpec {
    pub access_rules: Option<Vec<StorageFileFileStorageAccessRuleInput>>,
    pub description: Option<String>,
    pub name: String,
    pub size: i64,
}

/// `publicCloud.storage.file.FileStorageUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageFileFileStorageUpdate {
    pub checksum: String,
    pub target_spec: StorageFileFileStorageUpdateTargetSpec,
}

/// `publicCloud.storage.object.BucketEncryptionAlgorithmEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudStorageObjectBucketEncryptionAlgorithm {
    #[serde(rename = "AES256")]
    Aes256,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.storage.object.BucketEncryptionConfig`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectBucketEncryptionConfig {
    pub algorithm: PublicCloudStorageObjectBucketEncryptionAlgorithm,
}

/// `publicCloud.storage.object.BucketLocation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectBucketLocation {
    pub region: String,
}

/// `publicCloud.storage.object.BucketObjectLockModeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudStorageObjectBucketObjectLockMode {
    #[serde(rename = "COMPLIANCE")]
    Compliance,
    #[serde(rename = "GOVERNANCE")]
    Governance,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.storage.object.BucketObjectLockConfig`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectBucketObjectLockConfig {
    pub mode: PublicCloudStorageObjectBucketObjectLockMode,
    pub retention_days: i64,
    pub retention_years: Option<i64>,
}

/// `publicCloud.storage.object.BucketVersioningStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudStorageObjectBucketVersioningStatus {
    #[serde(rename = "DISABLED")]
    Disabled,
    #[serde(rename = "ENABLED")]
    Enabled,
    #[serde(rename = "SUSPENDED")]
    Suspended,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.storage.object.BucketVersioningConfig`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectBucketVersioningConfig {
    pub status: PublicCloudStorageObjectBucketVersioningStatus,
}

/// `publicCloud.storage.object.BucketCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectBucketCurrentState {
    pub encryption: Option<StorageObjectBucketEncryptionConfig>,
    pub location: Option<StorageObjectBucketLocation>,
    pub name: Option<String>,
    pub object_lock: Option<StorageObjectBucketObjectLockConfig>,
    #[serde(default)]
    pub tags: serde_json::Value,
    pub versioning: Option<StorageObjectBucketVersioningConfig>,
}

/// `publicCloud.storage.object.BucketTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectBucketTargetSpec {
    pub encryption: Option<StorageObjectBucketEncryptionConfig>,
    pub location: StorageObjectBucketLocation,
    pub name: String,
    pub object_lock: Option<StorageObjectBucketObjectLockConfig>,
    pub owner_user_id: Option<String>,
    #[serde(default)]
    pub tags: serde_json::Value,
    pub versioning: Option<StorageObjectBucketVersioningConfig>,
}

/// `publicCloud.storage.object.Bucket`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectBucket {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<StorageObjectBucketCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
    pub target_spec: Option<StorageObjectBucketTargetSpec>,
    pub updated_at: Option<String>,
}

/// `publicCloud.storage.object.BucketCreation`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectBucketCreation {
    pub target_spec: StorageObjectBucketTargetSpec,
}

/// `publicCloud.storage.object.BucketUpdateTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectBucketUpdateTargetSpec {
    pub encryption: Option<StorageObjectBucketEncryptionConfig>,
    pub object_lock: Option<StorageObjectBucketObjectLockConfig>,
    pub owner_user_id: Option<String>,
    #[serde(default)]
    pub tags: serde_json::Value,
    pub versioning: Option<StorageObjectBucketVersioningConfig>,
}

/// `publicCloud.storage.object.BucketUpdate`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectBucketUpdate {
    pub checksum: String,
    pub target_spec: StorageObjectBucketUpdateTargetSpec,
}

/// `publicCloud.storage.object.LifecycleRuleAbortIncompleteMultipartUpload`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectLifecycleRuleAbortIncompleteMultipartUpload {
    pub days_after_initiation: i64,
}

/// `publicCloud.storage.object.LifecycleRuleExpiration`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectLifecycleRuleExpiration {
    #[serde(default)]
    pub date: serde_json::Value,
    pub days: Option<i64>,
    pub expired_object_delete_marker: Option<bool>,
}

/// `publicCloud.storage.object.LifecycleRuleFilter`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectLifecycleRuleFilter {
    pub object_size_greater_than: Option<i64>,
    pub object_size_less_than: Option<i64>,
    pub prefix: Option<String>,
    #[serde(default)]
    pub tags: serde_json::Value,
}

/// `publicCloud.storage.object.LifecycleRuleNoncurrentVersionExpiration`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectLifecycleRuleNoncurrentVersionExpiration {
    pub newer_noncurrent_versions: Option<i64>,
    pub noncurrent_days: i64,
}

/// `publicCloud.storage.object.StorageClassEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudStorageObjectStorageClass {
    #[serde(rename = "DEEP_ARCHIVE")]
    DeepArchive,
    #[serde(rename = "GLACIER_IR")]
    GlacierIr,
    #[serde(rename = "HIGH_PERF")]
    HighPerf,
    #[serde(rename = "STANDARD")]
    Standard,
    #[serde(rename = "STANDARD_IA")]
    StandardIa,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.storage.object.LifecycleRuleNoncurrentVersionTransition`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectLifecycleRuleNoncurrentVersionTransition {
    pub newer_noncurrent_versions: Option<i64>,
    pub noncurrent_days: i64,
    pub storage_class: PublicCloudStorageObjectStorageClass,
}

/// `publicCloud.storage.object.LifecycleRuleStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudStorageObjectLifecycleRuleStatus {
    #[serde(rename = "DISABLED")]
    Disabled,
    #[serde(rename = "ENABLED")]
    Enabled,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.storage.object.LifecycleRuleTransition`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectLifecycleRuleTransition {
    pub date: Option<String>,
    pub days: Option<i64>,
    pub storage_class: PublicCloudStorageObjectStorageClass,
}

/// `publicCloud.storage.object.LifecycleRule`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectLifecycleRule {
    pub abort_incomplete_multipart_upload:
        Option<StorageObjectLifecycleRuleAbortIncompleteMultipartUpload>,
    pub expiration: Option<StorageObjectLifecycleRuleExpiration>,
    pub filter: Option<StorageObjectLifecycleRuleFilter>,
    pub id: String,
    pub noncurrent_version_expiration:
        Option<StorageObjectLifecycleRuleNoncurrentVersionExpiration>,
    pub noncurrent_version_transitions:
        Option<Vec<StorageObjectLifecycleRuleNoncurrentVersionTransition>>,
    pub status: PublicCloudStorageObjectLifecycleRuleStatus,
    pub transitions: Option<Vec<StorageObjectLifecycleRuleTransition>>,
}

/// `publicCloud.storage.object.LifecycleRulePut`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectLifecycleRulePut {
    #[serde(default)]
    pub rules: Vec<StorageObjectLifecycleRule>,
}

/// `publicCloud.storage.object.LifecycleRulesResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectLifecycleRulesResponse {
    pub rules: Option<Vec<StorageObjectLifecycleRule>>,
}

/// `publicCloud.storage.object.ReplicationRuleDestination`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectReplicationRuleDestination {
    pub name: String,
    pub region: String,
    pub storage_class: Option<PublicCloudStorageObjectStorageClass>,
}

/// `publicCloud.storage.object.ReplicationRuleFilter`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectReplicationRuleFilter {
    pub prefix: Option<String>,
    #[serde(default)]
    pub tags: serde_json::Value,
}

/// `publicCloud.storage.object.ReplicationRuleStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicCloudStorageObjectReplicationRuleStatus {
    #[serde(rename = "DISABLED")]
    Disabled,
    #[serde(rename = "ENABLED")]
    Enabled,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `publicCloud.storage.object.ReplicationRule`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectReplicationRule {
    pub delete_marker_replication: PublicCloudStorageObjectReplicationRuleStatus,
    pub destination: StorageObjectReplicationRuleDestination,
    pub filter: Option<StorageObjectReplicationRuleFilter>,
    pub id: String,
    pub priority: i64,
    pub status: PublicCloudStorageObjectReplicationRuleStatus,
}

/// `publicCloud.storage.object.ReplicationRulePut`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectReplicationRulePut {
    #[serde(default)]
    pub rules: Vec<StorageObjectReplicationRule>,
}

/// `publicCloud.storage.object.ReplicationRulesResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageObjectReplicationRulesResponse {
    pub rules: Option<Vec<StorageObjectReplicationRule>>,
}

impl OvhClient {
    /// `GET /publicCloud/project` — List all Public Cloud projects
    pub async fn public_cloud_projects(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<ProjectProjectAsyncWithIAM>> {
        self.get_page(
            &Self::append_query("/publicCloud/project", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /publicCloud/project/{projectId}` — Get details on a Public Cloud project
    pub async fn public_cloud_project(
        &self,
        project_id: &str,
    ) -> Result<ProjectProjectAsyncWithIAM> {
        self.get(&format!(
            "/publicCloud/project/{}",
            percent_encode(project_id)
        ))
        .await
    }

    /// `GET /publicCloud/project/{projectId}/rancher` — List managed Rancher services
    pub async fn public_cloud_project_rancher(
        &self,
        project_id: &str,
        page: &PageParams,
    ) -> Result<Vec<RancherRancher>> {
        self.get_page(
            &format!(
                "/publicCloud/project/{}/rancher",
                percent_encode(project_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `POST /publicCloud/project/{projectId}/rancher` — Create a new managed Rancher service
    pub async fn public_cloud_project_rancher_post(
        &self,
        project_id: &str,
        body: &RancherRancherCreation,
    ) -> Result<RancherRancher> {
        self.post_v2(
            &format!(
                "/publicCloud/project/{}/rancher",
                percent_encode(project_id)
            ),
            body,
            V2RequestOptions::default(),
        )
        .await
    }

    /// `DELETE /publicCloud/project/{projectId}/rancher/{rancherId}` — Delete a managed Rancher service
    pub async fn public_cloud_project_rancher_delete(
        &self,
        project_id: &str,
        rancher_id: &str,
    ) -> Result<RancherRancher> {
        self.delete_json(&format!(
            "/publicCloud/project/{}/rancher/{}",
            percent_encode(project_id),
            percent_encode(rancher_id)
        ))
        .await
    }

    /// `GET /publicCloud/project/{projectId}/rancher/{rancherId}` — Get a managed Rancher service
    pub async fn public_cloud_project_rancher_get(
        &self,
        project_id: &str,
        rancher_id: &str,
    ) -> Result<RancherRancher> {
        self.get(&format!(
            "/publicCloud/project/{}/rancher/{}",
            percent_encode(project_id),
            percent_encode(rancher_id)
        ))
        .await
    }

    /// `PUT /publicCloud/project/{projectId}/rancher/{rancherId}` — Update an existing managed Rancher service
    pub async fn public_cloud_project_rancher_put(
        &self,
        project_id: &str,
        rancher_id: &str,
        body: &RancherRancherUpdate,
    ) -> Result<RancherRancher> {
        self.put_json(
            &format!(
                "/publicCloud/project/{}/rancher/{}",
                percent_encode(project_id),
                percent_encode(rancher_id)
            ),
            body,
        )
        .await
    }

    /// `POST /publicCloud/project/{projectId}/rancher/{rancherId}/adminCredentials` — Reset the admin password
    pub async fn public_cloud_project_rancher_admin_credentials_post(
        &self,
        project_id: &str,
        rancher_id: &str,
    ) -> Result<RancherCredentials> {
        self.post_v2_no_body(
            &format!(
                "/publicCloud/project/{}/rancher/{}/adminCredentials",
                percent_encode(project_id),
                percent_encode(rancher_id)
            ),
            V2RequestOptions::default(),
        )
        .await
    }

    /// `GET /publicCloud/project/{projectId}/rancher/{rancherId}/capabilities/plan` — List available and current plans for the given managed Rancher service
    pub async fn public_cloud_project_rancher_capabilities_plan(
        &self,
        project_id: &str,
        rancher_id: &str,
        page: &PageParams,
    ) -> Result<Vec<RancherPlanCapability>> {
        self.get_page(
            &format!(
                "/publicCloud/project/{}/rancher/{}/capabilities/plan",
                percent_encode(project_id),
                percent_encode(rancher_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /publicCloud/project/{projectId}/rancher/{rancherId}/capabilities/version` — List available and current versions for the given managed Rancher service
    pub async fn public_cloud_project_rancher_capabilities_version(
        &self,
        project_id: &str,
        rancher_id: &str,
        page: &PageParams,
    ) -> Result<Vec<RancherVersionCapability>> {
        self.get_page(
            &format!(
                "/publicCloud/project/{}/rancher/{}/capabilities/version",
                percent_encode(project_id),
                percent_encode(rancher_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /publicCloud/project/{projectId}/rancher/{rancherId}/event` — List all events related to the managed Rancher service
    pub async fn public_cloud_project_rancher_event(
        &self,
        project_id: &str,
        rancher_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Event>> {
        self.get_page(
            &format!(
                "/publicCloud/project/{}/rancher/{}/event",
                percent_encode(project_id),
                percent_encode(rancher_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /publicCloud/project/{projectId}/rancher/{rancherId}/task` — List all asynchronous operations related to the managed Rancher service
    pub async fn public_cloud_project_rancher_task(
        &self,
        project_id: &str,
        rancher_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Task>> {
        self.get_page(
            &format!(
                "/publicCloud/project/{}/rancher/{}/task",
                percent_encode(project_id),
                percent_encode(rancher_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /publicCloud/project/{projectId}/rancher/{rancherId}/task/{taskId}` — Get a specific task related to the managed Rancher service
    pub async fn public_cloud_project_rancher_task_get(
        &self,
        project_id: &str,
        rancher_id: &str,
        task_id: &str,
    ) -> Result<Task> {
        self.get(&format!(
            "/publicCloud/project/{}/rancher/{}/task/{}",
            percent_encode(project_id),
            percent_encode(rancher_id),
            percent_encode(task_id)
        ))
        .await
    }

    /// `GET /publicCloud/project/{projectId}/reference/rancher/eligibility` — List available eligibility for creating a managed Rancher service
    pub async fn public_cloud_project_reference_rancher_eligibility(
        &self,
        project_id: &str,
    ) -> Result<RancherEligibilityReference> {
        self.get(&format!(
            "/publicCloud/project/{}/reference/rancher/eligibility",
            percent_encode(project_id)
        ))
        .await
    }

    /// `GET /publicCloud/project/{projectId}/reference/rancher/plan` — List available plans for creating a managed Rancher service
    pub async fn public_cloud_project_reference_rancher_plan(
        &self,
        project_id: &str,
        page: &PageParams,
    ) -> Result<Vec<RancherPlanReference>> {
        self.get_page(
            &format!(
                "/publicCloud/project/{}/reference/rancher/plan",
                percent_encode(project_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /publicCloud/project/{projectId}/reference/rancher/version` — List available versions for creating a managed Rancher service
    pub async fn public_cloud_project_reference_rancher_version(
        &self,
        project_id: &str,
        page: &PageParams,
    ) -> Result<Vec<RancherVersionReference>> {
        self.get_page(
            &format!(
                "/publicCloud/project/{}/reference/rancher/version",
                percent_encode(project_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /publicCloud/project/{projectId}/reference/region` — List available regions
    pub async fn public_cloud_project_reference_region(
        &self,
        project_id: &str,
        page: &PageParams,
    ) -> Result<Vec<ReferenceRegion>> {
        self.get_page(
            &format!(
                "/publicCloud/project/{}/reference/region",
                percent_encode(project_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /publicCloud/project/{projectId}/reference/region/{name}` — Get a region
    pub async fn public_cloud_project_reference_region_get(
        &self,
        name: &str,
        project_id: &str,
    ) -> Result<ReferenceRegion> {
        self.get(&format!(
            "/publicCloud/project/{}/reference/region/{}",
            percent_encode(project_id),
            percent_encode(name)
        ))
        .await
    }

    /// `GET /publicCloud/reference/rancher/plan` — List available plans for creating a managed Rancher service
    pub async fn public_cloud_reference_rancher_plans(
        &self,
        page: &PageParams,
    ) -> Result<Vec<RancherPlanReference>> {
        self.get_page("/publicCloud/reference/rancher/plan", &[], page)
            .await
            .map(|p| p.items)
    }

    /// `GET /publicCloud/reference/rancher/version` — List available versions for creating a managed Rancher service
    pub async fn public_cloud_reference_rancher_versions(
        &self,
        page: &PageParams,
    ) -> Result<Vec<RancherVersionReference>> {
        self.get_page("/publicCloud/reference/rancher/version", &[], page)
            .await
            .map(|p| p.items)
    }
}
