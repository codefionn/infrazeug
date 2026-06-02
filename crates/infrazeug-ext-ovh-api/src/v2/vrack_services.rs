//! OVHcloud API v2 **vrackServices** bindings (`/v2/vrackServices`).
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

/// `vrackServices.EligibleManagedService`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EligibleManagedService {
    pub managed_service_type: Option<String>,
    pub managed_service_urns: Option<Vec<String>>,
}

/// `vrackServices.Endpoint`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub description: Option<String>,
    #[serde(default)]
    pub ip: serde_json::Value,
}

/// `vrackServices.ProductStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VrackServicesProductStatus {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "DRAFT")]
    Draft,
    #[serde(rename = "SUSPENDED")]
    Suspended,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `vrackServices.Region`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub name: Option<String>,
}

/// `vrackServices.ResourceStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VrackServicesResourceStatus {
    #[serde(rename = "CREATING")]
    Creating,
    #[serde(rename = "DELETING")]
    Deleting,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "READY")]
    Ready,
    #[serde(rename = "UPDATING")]
    Updating,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `vrackServices.ServiceEndpoint`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEndpoint {
    pub endpoints: Option<Vec<Endpoint>>,
    pub managed_service_urn: Option<String>,
}

/// `vrackServices.ServiceRange`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRange {
    #[serde(default)]
    pub cidr: serde_json::Value,
    pub remaining_ips: Option<i64>,
    pub reserved_ips: Option<i64>,
    pub used_ips: Option<i64>,
}

/// `vrackServices.Subnet`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subnet {
    #[serde(default)]
    pub cidr: serde_json::Value,
    pub display_name: Option<String>,
    pub service_endpoints: Option<Vec<ServiceEndpoint>>,
    pub service_range: Option<ServiceRange>,
    pub vlan: Option<i64>,
}

/// `vrackServices.TargetServiceEndpoint`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetServiceEndpoint {
    pub managed_service_urn: String,
}

/// `vrackServices.TargetServiceRange`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetServiceRange {
    #[serde(default)]
    pub cidr: serde_json::Value,
}

/// `vrackServices.TargetSubnet`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetSubnet {
    #[serde(default)]
    pub cidr: serde_json::Value,
    pub display_name: Option<String>,
    #[serde(default)]
    pub service_endpoints: Vec<TargetServiceEndpoint>,
    pub service_range: TargetServiceRange,
    pub vlan: Option<i64>,
}

/// `vrackServices.VrackServicesCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VrackServicesCurrentState {
    pub product_status: Option<VrackServicesProductStatus>,
    pub region: Option<String>,
    pub subnets: Option<Vec<Subnet>>,
    pub vrack_id: Option<String>,
}

/// `vrackServices.VrackServicesTargetSpec`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VrackServicesTargetSpec {
    #[serde(default)]
    pub subnets: Vec<TargetSubnet>,
}

/// `vrackServices.VrackServices`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VrackServices {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<VrackServicesCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub id: Option<String>,
    pub resource_status: Option<VrackServicesResourceStatus>,
    pub target_spec: Option<VrackServicesTargetSpec>,
    pub updated_at: Option<String>,
}

/// `vrackServices.VrackServicesInput`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VrackServicesInput {
    pub checksum: String,
    pub target_spec: VrackServicesTargetSpec,
}

/// `vrackServices.VrackServicesWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VrackServicesWithIAM {
    pub checksum: Option<String>,
    pub created_at: Option<String>,
    pub current_state: Option<VrackServicesCurrentState>,
    pub current_tasks: Option<Vec<CurrentTask>>,
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub id: Option<String>,
    pub resource_status: Option<VrackServicesResourceStatus>,
    pub target_spec: Option<VrackServicesTargetSpec>,
    pub updated_at: Option<String>,
}

impl OvhClient {
    /// `GET /vrackServices/reference/compatibleManagedServiceType` — List all managed service types that are compatible with vRack Services (IAM resource types)
    pub async fn vrack_services_reference_compatible_managed_service_types(
        &self,
        page: &PageParams,
    ) -> Result<Vec<String>> {
        self.get_page(
            "/vrackServices/reference/compatibleManagedServiceType",
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /vrackServices/reference/region` — List all vRack Services regions
    pub async fn vrack_services_reference_regions(&self, page: &PageParams) -> Result<Vec<Region>> {
        self.get_page("/vrackServices/reference/region", &[], page)
            .await
            .map(|p| p.items)
    }

    /// `GET /vrackServices/resource` — List all vRack Services
    pub async fn vrack_services_resources(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<VrackServicesWithIAM>> {
        self.get_page(
            &Self::append_query("/vrackServices/resource", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /vrackServices/resource/{vrackServicesId}` — Retrieve a vRack Services
    pub async fn vrack_services_resource(
        &self,
        vrack_services_id: &str,
    ) -> Result<VrackServicesWithIAM> {
        self.get(&format!(
            "/vrackServices/resource/{}",
            percent_encode(vrack_services_id)
        ))
        .await
    }

    /// `PUT /vrackServices/resource/{vrackServicesId}` — Request updates on the vRack Services configuration
    pub async fn vrack_services_resource_put(
        &self,
        vrack_services_id: &str,
        body: &VrackServicesInput,
    ) -> Result<VrackServices> {
        self.put_json(
            &format!(
                "/vrackServices/resource/{}",
                percent_encode(vrack_services_id)
            ),
            body,
        )
        .await
    }

    /// `GET /vrackServices/resource/{vrackServicesId}/eligibleManagedService` — List every managed service eligible to the requested vRack Services
    pub async fn vrack_services_resource_eligible_managed_service(
        &self,
        vrack_services_id: &str,
        page: &PageParams,
    ) -> Result<Vec<EligibleManagedService>> {
        self.get_page(
            &format!(
                "/vrackServices/resource/{}/eligibleManagedService",
                percent_encode(vrack_services_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /vrackServices/resource/{vrackServicesId}/task` — List all asynchronous operations related to the vRack Services
    pub async fn vrack_services_resource_task(
        &self,
        vrack_services_id: &str,
        page: &PageParams,
    ) -> Result<Vec<Task>> {
        self.get_page(
            &format!(
                "/vrackServices/resource/{}/task",
                percent_encode(vrack_services_id)
            ),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /vrackServices/resource/{vrackServicesId}/task/{taskId}` — Get the task
    pub async fn vrack_services_resource_task_get(
        &self,
        task_id: &str,
        vrack_services_id: &str,
    ) -> Result<Task> {
        self.get(&format!(
            "/vrackServices/resource/{}/task/{}",
            percent_encode(vrack_services_id),
            percent_encode(task_id)
        ))
        .await
    }
}
