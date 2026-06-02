//! The `/backupServices` product surface (v2 branch).
//!
//! Backup Services follows the v2 resource shape: every resource exposes a
//! read-only `current_state`, a writable `target_spec`, a `resource_status`
//! and any in-flight `current_tasks`. The hierarchy is:
//!
//! ```text
//! tenant
//! ├── vault
//! └── vspc (Veeam Service Provider Console tenant)
//!     ├── backupAgent
//!     └── backupPolicies / managementAgent
//! ```
//!
//! List endpoints are cursor-paginated (see [`Page`](crate::Page)); each
//! `*_page` method returns one page and each plain list method follows the
//! cursor to completion.
//!
//! Models mirror the schema at `GET /v2/backupServices.json`. Enums carry an
//! `Other` catch-all so unrecognised server values still deserialize. Regions
//! are kept as plain strings because OVH adds new ones frequently.

use crate::client::{percent_encode, OvhClient, Page, PageParams};
use crate::error::Result;
use crate::iam::ResourceMetadata;
use serde::{Deserialize, Serialize};

/// v2 branch prefix for backupServices routes.
const BASE: &str = "/v2/backupServices";

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Lifecycle status of a v2 resource (`common.ResourceStatusEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceStatus {
    Creating,
    Deleting,
    Error,
    OutOfSync,
    Ready,
    Suspended,
    /// The server's own `UNKNOWN` status.
    Unknown,
    Updating,
    /// A value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// Status of an asynchronous task (`common.CurrentTaskStatusEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CurrentTaskStatus {
    Error,
    Pending,
    Running,
    Scheduled,
    WaitingUserInput,
    /// A value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// Backup vault billing type (`backup.VaultTypeEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VaultType {
    Bundle,
    Paygo,
    /// A value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// Performance tier of a bucket/vault (`backup.BucketPerformanceEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BucketPerformance {
    HighPerf,
    Standard,
    /// A value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// Role a bucket plays in replication (`backup.BucketRoleEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BucketRole {
    Primary,
    Replica,
    /// A value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// Status of a backup agent (`backup.AgentStatusEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentStatus {
    Creating,
    Disabled,
    Enabled,
    NotConfigured,
    NotInstalled,
    Updating,
    /// A value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// Environment a backup agent protects (`backup.AgentProductTypeEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentProductType {
    OnPremise,
    OvhcloudBaremetal,
    OvhcloudPublicCloud,
    OvhcloudVps,
    /// A value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

// ---------------------------------------------------------------------------
// Common task types
// ---------------------------------------------------------------------------

/// An error reported by an asynchronous task (`common.TaskError`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskError {
    /// Error message.
    pub message: String,
}

/// An asynchronous operation currently running on a resource
/// (`common.CurrentTask`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentTask {
    /// Errors that occurred on the task, if any.
    #[serde(default)]
    pub errors: Option<Vec<TaskError>>,
    /// Task UUID.
    pub id: String,
    /// Link to follow the task.
    pub link: String,
    /// Current status.
    #[serde(default)]
    pub status: Option<CurrentTaskStatus>,
    /// Task type. Renamed from the JSON `type` field.
    #[serde(rename = "type")]
    pub kind: String,
}

// ---------------------------------------------------------------------------
// Tenant
// ---------------------------------------------------------------------------

/// A backup tenant (`backup.tenant` / `backup.tenantWithIAM`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tenant {
    /// Creation timestamp.
    pub created_at: String,
    /// Observed state.
    pub current_state: TenantCurrentState,
    /// In-flight tasks.
    #[serde(default)]
    pub current_tasks: Vec<CurrentTask>,
    /// Tenant UUID.
    pub id: String,
    /// Lifecycle status.
    pub resource_status: ResourceStatus,
    /// Desired state.
    pub target_spec: TenantTargetSpec,
    /// Last update timestamp.
    pub updated_at: String,
    /// IAM metadata (present on `WithIAM` responses).
    #[serde(default)]
    pub iam: Option<ResourceMetadata>,
}

/// Observed state of a tenant (`backup.tenant.CurrentState`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TenantCurrentState {
    /// Tenant UUID.
    pub id: String,
    /// Tenant name.
    pub name: String,
    /// IDs of vaults under the tenant.
    #[serde(default)]
    pub vaults: Vec<String>,
    /// IDs of VSPC tenants under the tenant.
    #[serde(default)]
    pub vspc_tenants: Vec<String>,
}

/// Desired state of a tenant (`backup.tenant.TargetSpec`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantTargetSpec {
    /// Tenant name.
    pub name: String,
}

// ---------------------------------------------------------------------------
// Vault
// ---------------------------------------------------------------------------

/// A backup vault (`backup.tenant.vault` / `…vaultWithIAM`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vault {
    /// Creation timestamp.
    pub created_at: String,
    /// Observed state.
    pub current_state: VaultCurrentState,
    /// In-flight tasks.
    #[serde(default)]
    pub current_tasks: Vec<CurrentTask>,
    /// Vault UUID.
    pub id: String,
    /// Lifecycle status.
    pub resource_status: ResourceStatus,
    /// Desired state.
    pub target_spec: VaultTargetSpec,
    /// Last update timestamp.
    pub updated_at: String,
    /// IAM metadata (present on `WithIAM` responses).
    #[serde(default)]
    pub iam: Option<ResourceMetadata>,
}

/// Observed state of a vault (`backup.tenant.vault.CurrentState`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultCurrentState {
    /// Storage buckets backing the vault.
    #[serde(default)]
    pub buckets: Vec<Bucket>,
    /// Vault UUID.
    pub id: String,
    /// Vault name.
    pub name: String,
    /// OVH region code (`common.RegionEnum`).
    pub region: String,
    /// Resource name (URN-style identifier).
    pub resource_name: String,
    /// Lifecycle status.
    pub status: ResourceStatus,
    /// Billing type. Renamed from the JSON `type` field.
    #[serde(rename = "type")]
    pub kind: VaultType,
    /// IDs of VSPC tenants linked to the vault.
    #[serde(default)]
    pub vspc_tenants: Vec<String>,
}

/// A storage bucket backing a vault (`backup.tenant.vault.bucket`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    /// Bucket UUID.
    pub id: String,
    /// Bucket name.
    pub name: String,
    /// Performance tier.
    pub performance: BucketPerformance,
    /// OVH region code.
    pub region: String,
    /// Replication role.
    pub role: BucketRole,
    /// Lifecycle status.
    pub status: ResourceStatus,
}

/// Desired state of a vault (`backup.tenant.vault.TargetSpec`); also the body
/// of `PUT …/vault/{vaultId}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultTargetSpec {
    /// Vault name.
    pub name: String,
}

// ---------------------------------------------------------------------------
// VSPC tenant
// ---------------------------------------------------------------------------

/// A Veeam Service Provider Console tenant (`backup.tenant.vspc` / `…WithIAM`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vspc {
    /// Creation timestamp.
    pub created_at: String,
    /// Observed state.
    pub current_state: VspcCurrentState,
    /// In-flight tasks.
    #[serde(default)]
    pub current_tasks: Vec<CurrentTask>,
    /// VSPC tenant UUID.
    pub id: String,
    /// Lifecycle status.
    pub resource_status: ResourceStatus,
    /// Desired state.
    pub target_spec: VspcTargetSpec,
    /// Last update timestamp.
    pub updated_at: String,
    /// IAM metadata (present on `WithIAM` responses).
    #[serde(default)]
    pub iam: Option<ResourceMetadata>,
}

/// Observed state of a VSPC tenant (`backup.tenant.vspc.CurrentState`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VspcCurrentState {
    /// Console access URL.
    pub access_url: String,
    /// Backup agents registered against the tenant.
    #[serde(default)]
    pub backup_agents: Vec<BackupAgentCurrentState>,
    /// Company name shown in the console.
    pub company_name: String,
    /// VSPC tenant UUID.
    pub id: String,
    /// VSPC tenant name.
    pub name: String,
    /// OVH region code.
    pub region: String,
    /// Lifecycle status.
    pub status: ResourceStatus,
    /// Vaults linked to the tenant.
    #[serde(default)]
    pub vaults: Vec<VspcVault>,
}

/// Desired state of a VSPC tenant (`backup.tenant.vspc.TargetSpec`); also the
/// body of `PUT …/vspc/{vspcTenantId}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VspcTargetSpec {
    /// VSPC tenant name.
    pub name: String,
}

/// A vault linked to a VSPC tenant (`backup.tenant.vspc.vault`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VspcVault {
    /// IP blocks allowed to reach the vault.
    #[serde(default)]
    pub allowed_ips: Vec<String>,
    /// Vault UUID.
    pub id: String,
    /// Vault name.
    pub name: String,
    /// Performance tier.
    pub performance: BucketPerformance,
    /// OVH region code.
    pub region: String,
    /// Resource name.
    pub resource_name: String,
    /// Lifecycle status.
    pub status: ResourceStatus,
    /// Billing type. Renamed from the JSON `type` field.
    #[serde(rename = "type")]
    pub kind: VaultType,
}

// ---------------------------------------------------------------------------
// Backup agent
// ---------------------------------------------------------------------------

/// A VSPC backup agent (`backup.tenant.vspc.backupAgent`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupAgent {
    /// Creation timestamp.
    pub created_at: String,
    /// Observed state.
    pub current_state: BackupAgentCurrentState,
    /// In-flight tasks.
    #[serde(default)]
    pub current_tasks: Vec<CurrentTask>,
    /// Agent UUID.
    pub id: String,
    /// Agent status.
    pub status: AgentStatus,
    /// Desired state.
    pub target_spec: BackupAgentTargetSpec,
    /// Last update timestamp.
    pub updated_at: String,
}

/// Observed state of a backup agent
/// (`backup.tenant.vspc.backupAgent.CurrentState`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupAgentCurrentState {
    /// Agent UUID.
    pub id: String,
    /// IP blocks of the protected environment.
    #[serde(default)]
    pub ips: Vec<String>,
    /// Agent name.
    pub name: String,
    /// Applied backup policy.
    pub policy: String,
    /// Resource name of the protected product.
    pub product_resource_name: String,
    /// Protected environment type. Renamed from the JSON `type` field.
    #[serde(rename = "type")]
    pub kind: AgentProductType,
    /// Vault the agent backs up to, if assigned.
    #[serde(default)]
    pub vault_id: Option<String>,
}

/// Desired state of a backup agent
/// (`backup.tenant.vspc.backupAgent.TargetSpec`); body of
/// `PUT …/backupAgent/{backupAgentId}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupAgentTargetSpec {
    /// Display name.
    pub display_name: String,
    /// IP blocks of the protected environment.
    #[serde(default)]
    pub ips: Vec<String>,
    /// Backup policy to apply.
    pub policy: String,
}

/// Spec for creating a backup agent
/// (`backup.tenant.vspc.backupAgent.CreateSpec`); body of
/// `POST …/backupAgent`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupAgentCreateSpec {
    /// Display name.
    pub display_name: String,
    /// IP blocks of the protected environment.
    #[serde(default)]
    pub ips: Vec<String>,
    /// Resource name of the product to protect.
    pub product_resource_name: String,
    /// OVH region code to deploy in.
    pub region: String,
}

/// Download links for the VSPC management agent
/// (`backup.tenant.vspc.managementAgent`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagementAgent {
    /// Linux deploy script, if provided.
    #[serde(default)]
    pub linux_deploy_script: Option<String>,
    /// Linux installer URL.
    pub linux_url: String,
    /// macOS installer URL.
    pub mac_url: String,
    /// Windows installer URL.
    pub windows_url: String,
}

// ---------------------------------------------------------------------------
// API methods
// ---------------------------------------------------------------------------

impl OvhClient {
    /// `GET /backupServices/tenant` — one page of backup tenants.
    pub async fn backup_tenants_page(&self, page: &PageParams) -> Result<Page<Tenant>> {
        self.get_page(&format!("{BASE}/tenant"), &[], page).await
    }

    /// `GET /backupServices/tenant` — every backup tenant (follows the cursor).
    pub async fn backup_tenants(&self) -> Result<Vec<Tenant>> {
        self.get_all(&format!("{BASE}/tenant"), &[]).await
    }

    /// `GET /backupServices/tenant/{backupServicesId}` — one backup tenant.
    pub async fn backup_tenant(&self, tenant_id: &str) -> Result<Tenant> {
        self.get(&format!("{BASE}/tenant/{}", percent_encode(tenant_id)))
            .await
    }

    /// `GET …/tenant/{id}/vault` — one page of vaults.
    pub async fn backup_vaults_page(
        &self,
        tenant_id: &str,
        page: &PageParams,
    ) -> Result<Page<Vault>> {
        self.get_page(
            &format!("{BASE}/tenant/{}/vault", percent_encode(tenant_id)),
            &[],
            page,
        )
        .await
    }

    /// `GET …/tenant/{id}/vault` — every vault (follows the cursor).
    pub async fn backup_vaults(&self, tenant_id: &str) -> Result<Vec<Vault>> {
        self.get_all(
            &format!("{BASE}/tenant/{}/vault", percent_encode(tenant_id)),
            &[],
        )
        .await
    }

    /// `GET …/vault/{vaultId}` — one vault.
    pub async fn backup_vault(&self, tenant_id: &str, vault_id: &str) -> Result<Vault> {
        self.get(&format!(
            "{BASE}/tenant/{}/vault/{}",
            percent_encode(tenant_id),
            percent_encode(vault_id)
        ))
        .await
    }

    /// `PUT …/vault/{vaultId}` — update a vault, returning the new state.
    pub async fn backup_update_vault(
        &self,
        tenant_id: &str,
        vault_id: &str,
        spec: &VaultTargetSpec,
    ) -> Result<Vault> {
        self.put_json(
            &format!(
                "{BASE}/tenant/{}/vault/{}",
                percent_encode(tenant_id),
                percent_encode(vault_id)
            ),
            spec,
        )
        .await
    }

    /// `GET …/tenant/{id}/vspc` — one page of VSPC tenants.
    pub async fn backup_vspc_tenants_page(
        &self,
        tenant_id: &str,
        page: &PageParams,
    ) -> Result<Page<Vspc>> {
        self.get_page(
            &format!("{BASE}/tenant/{}/vspc", percent_encode(tenant_id)),
            &[],
            page,
        )
        .await
    }

    /// `GET …/tenant/{id}/vspc` — every VSPC tenant (follows the cursor).
    pub async fn backup_vspc_tenants(&self, tenant_id: &str) -> Result<Vec<Vspc>> {
        self.get_all(
            &format!("{BASE}/tenant/{}/vspc", percent_encode(tenant_id)),
            &[],
        )
        .await
    }

    /// `GET …/vspc/{vspcTenantId}` — one VSPC tenant.
    pub async fn backup_vspc_tenant(&self, tenant_id: &str, vspc_id: &str) -> Result<Vspc> {
        self.get(&format!(
            "{BASE}/tenant/{}/vspc/{}",
            percent_encode(tenant_id),
            percent_encode(vspc_id)
        ))
        .await
    }

    /// `PUT …/vspc/{vspcTenantId}` — update a VSPC tenant, returning new state.
    pub async fn backup_update_vspc_tenant(
        &self,
        tenant_id: &str,
        vspc_id: &str,
        spec: &VspcTargetSpec,
    ) -> Result<Vspc> {
        self.put_json(
            &format!(
                "{BASE}/tenant/{}/vspc/{}",
                percent_encode(tenant_id),
                percent_encode(vspc_id)
            ),
            spec,
        )
        .await
    }

    /// `GET …/vspc/{vspcTenantId}/backupAgent` — one page of backup agents.
    pub async fn backup_agents_page(
        &self,
        tenant_id: &str,
        vspc_id: &str,
        page: &PageParams,
    ) -> Result<Page<BackupAgent>> {
        self.get_page(
            &format!(
                "{BASE}/tenant/{}/vspc/{}/backupAgent",
                percent_encode(tenant_id),
                percent_encode(vspc_id)
            ),
            &[],
            page,
        )
        .await
    }

    /// `GET …/vspc/{vspcTenantId}/backupAgent` — every agent (follows cursor).
    pub async fn backup_agents(&self, tenant_id: &str, vspc_id: &str) -> Result<Vec<BackupAgent>> {
        self.get_all(
            &format!(
                "{BASE}/tenant/{}/vspc/{}/backupAgent",
                percent_encode(tenant_id),
                percent_encode(vspc_id)
            ),
            &[],
        )
        .await
    }

    /// `POST …/vspc/{vspcTenantId}/backupAgent` — create a backup agent.
    pub async fn backup_create_agent(
        &self,
        tenant_id: &str,
        vspc_id: &str,
        spec: &BackupAgentCreateSpec,
    ) -> Result<()> {
        self.post_void(
            &format!(
                "{BASE}/tenant/{}/vspc/{}/backupAgent",
                percent_encode(tenant_id),
                percent_encode(vspc_id)
            ),
            spec,
        )
        .await
    }

    /// `GET …/backupAgent/{backupAgentId}` — one backup agent.
    pub async fn backup_agent(
        &self,
        tenant_id: &str,
        vspc_id: &str,
        agent_id: &str,
    ) -> Result<BackupAgent> {
        self.get(&format!(
            "{BASE}/tenant/{}/vspc/{}/backupAgent/{}",
            percent_encode(tenant_id),
            percent_encode(vspc_id),
            percent_encode(agent_id)
        ))
        .await
    }

    /// `PUT …/backupAgent/{backupAgentId}` — update a backup agent.
    pub async fn backup_update_agent(
        &self,
        tenant_id: &str,
        vspc_id: &str,
        agent_id: &str,
        spec: &BackupAgentTargetSpec,
    ) -> Result<()> {
        self.put(
            &format!(
                "{BASE}/tenant/{}/vspc/{}/backupAgent/{}",
                percent_encode(tenant_id),
                percent_encode(vspc_id),
                percent_encode(agent_id)
            ),
            spec,
        )
        .await
    }

    /// `DELETE …/backupAgent/{backupAgentId}` — remove a backup agent.
    pub async fn backup_delete_agent(
        &self,
        tenant_id: &str,
        vspc_id: &str,
        agent_id: &str,
    ) -> Result<()> {
        self.delete(&format!(
            "{BASE}/tenant/{}/vspc/{}/backupAgent/{}",
            percent_encode(tenant_id),
            percent_encode(vspc_id),
            percent_encode(agent_id)
        ))
        .await
    }

    /// `GET …/vspc/{vspcTenantId}/backupPolicies` — one page of policy names.
    pub async fn backup_policies_page(
        &self,
        tenant_id: &str,
        vspc_id: &str,
        page: &PageParams,
    ) -> Result<Page<String>> {
        self.get_page(
            &format!(
                "{BASE}/tenant/{}/vspc/{}/backupPolicies",
                percent_encode(tenant_id),
                percent_encode(vspc_id)
            ),
            &[],
            page,
        )
        .await
    }

    /// `GET …/vspc/{vspcTenantId}/backupPolicies` — every policy name.
    pub async fn backup_policies(&self, tenant_id: &str, vspc_id: &str) -> Result<Vec<String>> {
        self.get_all(
            &format!(
                "{BASE}/tenant/{}/vspc/{}/backupPolicies",
                percent_encode(tenant_id),
                percent_encode(vspc_id)
            ),
            &[],
        )
        .await
    }

    /// `GET …/vspc/{vspcTenantId}/managementAgent` — management agent links.
    pub async fn backup_management_agent(
        &self,
        tenant_id: &str,
        vspc_id: &str,
    ) -> Result<ManagementAgent> {
        self.get(&format!(
            "{BASE}/tenant/{}/vspc/{}/managementAgent",
            percent_encode(tenant_id),
            percent_encode(vspc_id)
        ))
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_tenant_with_iam() {
        let json = r#"{
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-02-01T00:00:00Z",
            "id": "11111111-1111-1111-1111-111111111111",
            "resourceStatus": "READY",
            "currentTasks": [],
            "currentState": {
                "id": "11111111-1111-1111-1111-111111111111",
                "name": "my-tenant",
                "vaults": ["v1"],
                "vspcTenants": []
            },
            "targetSpec": {"name": "my-tenant"},
            "iam": {
                "id": "22222222-2222-2222-2222-222222222222",
                "urn": "urn:v1:eu:resource:backupServices:my-tenant"
            }
        }"#;
        let t: Tenant = serde_json::from_str(json).unwrap();
        assert_eq!(t.resource_status, ResourceStatus::Ready);
        assert_eq!(t.current_state.vaults, vec!["v1"]);
        assert_eq!(t.target_spec.name, "my-tenant");
        assert!(t.iam.is_some());
    }

    #[test]
    fn deserialize_vault_without_iam() {
        let json = r#"{
            "createdAt": "2024-01-01T00:00:00Z",
            "updatedAt": "2024-01-01T00:00:00Z",
            "id": "33333333-3333-3333-3333-333333333333",
            "resourceStatus": "UPDATING",
            "targetSpec": {"name": "vault-a"},
            "currentState": {
                "id": "33333333-3333-3333-3333-333333333333",
                "name": "vault-a",
                "region": "eu-west-par",
                "resourceName": "vault-a-rn",
                "status": "READY",
                "type": "PAYGO",
                "buckets": [
                    {"id":"b1","name":"bucket-1","performance":"HIGH_PERF","region":"eu-west-par","role":"PRIMARY","status":"READY"}
                ]
            }
        }"#;
        let v: Vault = serde_json::from_str(json).unwrap();
        assert!(v.iam.is_none());
        assert_eq!(v.current_state.kind, VaultType::Paygo);
        assert_eq!(v.current_state.buckets[0].role, BucketRole::Primary);
        assert_eq!(
            v.current_state.buckets[0].performance,
            BucketPerformance::HighPerf
        );
        assert!(v.current_tasks.is_empty());
    }

    #[test]
    fn unknown_status_is_other_but_real_unknown_is_unknown() {
        #[derive(Deserialize)]
        struct Holder {
            status: ResourceStatus,
        }
        let real: Holder = serde_json::from_str(r#"{"status":"UNKNOWN"}"#).unwrap();
        assert_eq!(real.status, ResourceStatus::Unknown);
        let novel: Holder = serde_json::from_str(r#"{"status":"WARP_SPEED"}"#).unwrap();
        assert_eq!(novel.status, ResourceStatus::Other);
    }

    #[test]
    fn target_spec_serializes_to_camel_case() {
        let spec = BackupAgentCreateSpec {
            display_name: "agent-1".into(),
            ips: vec!["203.0.113.0/24".into()],
            product_resource_name: "vps-123".into(),
            region: "eu-west-par".into(),
        };
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("\"displayName\":\"agent-1\""));
        assert!(json.contains("\"productResourceName\":\"vps-123\""));
        assert!(json.contains("\"region\":\"eu-west-par\""));
    }

    #[test]
    fn agent_product_type_round_trips() {
        assert_eq!(
            serde_json::to_string(&AgentProductType::OvhcloudPublicCloud).unwrap(),
            "\"OVHCLOUD_PUBLIC_CLOUD\""
        );
        let parsed: AgentProductType = serde_json::from_str("\"ON_PREMISE\"").unwrap();
        assert_eq!(parsed, AgentProductType::OnPremise);
    }
}
