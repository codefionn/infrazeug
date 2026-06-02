//! Registered domain names (`/domain`, `/domain/{serviceName}`).

use super::domain_path;
use crate::alldom::ServiceInfos;
use crate::client::OvhClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// DNSSEC state of a domain (`domain.DnssecStateEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DnssecState {
    Disabled,
    Enabled,
    #[serde(rename = "not_supported")]
    NotSupported,
}

/// Lifecycle state of a registered domain (`domain.DomainStateEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainState {
    AutorenewInProgress,
    AutorenewRegistryInProgress,
    Deleted,
    Dispute,
    Expired,
    Ok,
    OutgoingTransfer,
    PendingCreate,
    PendingDelete,
    PendingIncomingTransfer,
    PendingInstallation,
    RegistrySuspended,
    Restorable,
    TechnicalSuspended,
    #[serde(other)]
    Unknown,
}

/// Contact reference on a domain service (`domain.ContactSummary`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactSummary {
    pub id: String,
}

/// A registered domain name (`domain.DomainService`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainService {
    pub domain: String,
    pub service_id: i64,
    pub expiration_date: String,
    pub renewal_date: Option<String>,
    pub state: DomainState,
    pub dnssec_state: DnssecState,
    pub dnssec_supported: bool,
    pub contact_admin: ContactSummary,
    pub contact_billing: ContactSummary,
    pub contact_owner: ContactSummary,
    pub contact_tech: ContactSummary,
    #[serde(default)]
    pub whois_owner: Option<String>,
    #[serde(default)]
    pub suspension_state: Option<String>,
    #[serde(default)]
    pub transfer_lock_status: Option<String>,
    #[serde(default)]
    pub name_servers: Vec<NameServer>,
}

/// Name server attached to a domain (`domain.nameServer.NameServer`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameServer {
    #[serde(default)]
    pub id: Option<i64>,
    pub name_server: String,
    #[serde(default)]
    pub ipv4: Option<String>,
    #[serde(default)]
    pub ipv6: Option<String>,
    #[serde(default)]
    pub name_server_type: Option<String>,
}

/// Input for bulk DNS update (`domain.nameServer.NameServerInput`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameServerInput {
    pub host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
}

/// Body for `POST …/nameServers/update`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameServersUpdate {
    pub name_servers: Vec<NameServerInput>,
}

/// Domain operation task (`domain.Task`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainOperationTask {
    pub id: i64,
    pub function: String,
    pub status: String,
    pub creation_date: String,
    pub last_update: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub done_date: Option<String>,
    #[serde(default)]
    pub can_cancel: bool,
    #[serde(default)]
    pub can_accelerate: bool,
    #[serde(default)]
    pub can_relaunch: bool,
}

impl OvhClient {
    /// `GET /domain` — list registered domain names.
    pub async fn domains(&self) -> Result<Vec<String>> {
        self.get_v1("/domain").await
    }

    /// `GET /domain/{serviceName}` — domain service details.
    pub async fn domain_service(&self, service_name: &str) -> Result<DomainService> {
        let path = domain_path(self, service_name, "");
        self.get_v1(&path).await
    }

    /// `GET /domain/{serviceName}/serviceInfos` — billing and renewal metadata.
    pub async fn domain_service_infos(&self, service_name: &str) -> Result<ServiceInfos> {
        let path = domain_path(self, service_name, "/serviceInfos");
        self.get_v1(&path).await
    }

    /// `PUT /domain/{serviceName}/serviceInfos` — update renewal settings.
    pub async fn domain_update_service_infos(
        &self,
        service_name: &str,
        infos: &ServiceInfos,
    ) -> Result<()> {
        let path = domain_path(self, service_name, "/serviceInfos");
        self.put_v1(&path, infos).await
    }

    /// `POST /domain/{serviceName}/nameServers/update` — replace delegation NS.
    pub async fn domain_nameservers_update(
        &self,
        service_name: &str,
        update: &NameServersUpdate,
    ) -> Result<DomainOperationTask> {
        let path = domain_path(self, service_name, "/nameServers/update");
        self.post_v1(&path, update).await
    }

    /// `GET /domain/{serviceName}/nameServer` — list name server ids.
    pub async fn domain_nameserver_ids(&self, service_name: &str) -> Result<Vec<i64>> {
        let path = domain_path(self, service_name, "/nameServer");
        self.get_v1(&path).await
    }

    /// `GET /domain/{serviceName}/task` — list pending operation task ids.
    pub async fn domain_task_ids(&self, service_name: &str) -> Result<Vec<i64>> {
        let path = domain_path(self, service_name, "/task");
        self.get_v1(&path).await
    }

    /// `GET /domain/{serviceName}/task/{id}` — operation task details.
    pub async fn domain_task(
        &self,
        service_name: &str,
        task_id: i64,
    ) -> Result<DomainOperationTask> {
        let path = format!("{}/task/{task_id}", domain_path(self, service_name, ""));
        self.get_v1(&path).await
    }

    /// `POST /domain/{serviceName}/task/{id}/cancel`.
    pub async fn domain_task_cancel(&self, service_name: &str, task_id: i64) -> Result<()> {
        let path = format!(
            "{}/task/{task_id}/cancel",
            domain_path(self, service_name, "")
        );
        self.post_v1_void(&path).await
    }

    /// `POST /domain/{serviceName}/task/{id}/accelerate`.
    pub async fn domain_task_accelerate(&self, service_name: &str, task_id: i64) -> Result<()> {
        let path = format!(
            "{}/task/{task_id}/accelerate",
            domain_path(self, service_name, "")
        );
        self.post_v1_void(&path).await
    }

    /// `POST /domain/{serviceName}/task/{id}/relaunch`.
    pub async fn domain_task_relaunch(&self, service_name: &str, task_id: i64) -> Result<()> {
        let path = format!(
            "{}/task/{task_id}/relaunch",
            domain_path(self, service_name, "")
        );
        self.post_v1_void(&path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_domain_service() {
        let svc: DomainService = serde_json::from_str(
            r#"{
                "domain": "example.com",
                "serviceId": 1,
                "expirationDate": "2026-01-01T00:00:00+00:00",
                "state": "ok",
                "dnssecState": "disabled",
                "dnssecSupported": true,
                "contactAdmin": {"id": "ab1"},
                "contactBilling": {"id": "ab2"},
                "contactOwner": {"id": "ab3"},
                "contactTech": {"id": "ab4"},
                "nameServers": [{"nameServer": "dns1.example.net"}]
            }"#,
        )
        .unwrap();
        assert_eq!(svc.domain, "example.com");
    }
}
