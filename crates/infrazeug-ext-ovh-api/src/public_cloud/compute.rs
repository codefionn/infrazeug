//! Public Cloud **Compute** — instances (`/cloud/project/…/instance`).

use super::project_path;
use crate::client::OvhClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Instance lifecycle status (`cloud.instance.InstanceStatusEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InstanceStatus {
    Active,
    Build,
    Building,
    Deleted,
    Deleting,
    Error,
    HardReboot,
    Migrating,
    Password,
    Paused,
    Reboot,
    Rebuild,
    Rescue,
    Rescued,
    Rescuing,
    Resize,
    Resized,
    Resuming,
    RevertResize,
    Shelved,
    ShelvedOffloaded,
    Shelving,
    Shutoff,
    Snapshotting,
    SoftDeleted,
    Stopped,
    Suspended,
    Unknown,
    Unrescuing,
    Unshelving,
    VerifyResize,
    #[serde(other)]
    Other,
}

/// Reboot mode (`cloud.instance.RebootTypeEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RebootType {
    Hard,
    Soft,
}

/// Optional filter for `GET …/instance`.
#[derive(Debug, Clone, Default)]
pub struct InstanceListQuery<'a> {
    pub region: Option<&'a str>,
}

/// A compute instance (`cloud.instance.Instance` / list entries).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub region: String,
    pub flavor_id: String,
    #[serde(default)]
    pub image_id: Option<String>,
    #[serde(default)]
    pub availability_zone: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub status: Option<InstanceStatus>,
    #[serde(default)]
    pub flavor_name: Option<String>,
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default)]
    pub pending_task: Option<bool>,
    #[serde(default, alias = "addresses")]
    pub ip_addresses: Vec<InstanceIpAddress>,
}

/// Instance IP address (`cloud.instance.IpAddress`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceIpAddress {
    pub ip: String,
    #[serde(default)]
    pub gateway_ip: Option<String>,
    #[serde(default)]
    pub network_id: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub version: Option<i64>,
}

/// Body for `POST …/instance` (`cloud.ProjectInstanceCreation`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceCreate {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub flavor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly_billing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub networks: Option<Vec<InstanceNetworkParams>>,
}

/// Network attachment on instance create (`cloud.instance.NetworkParams`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceNetworkParams {
    pub network_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
}

/// Body for `PUT …/instance/{instanceId}` (`cloud.ProjectInstanceUpdate`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceUpdate {
    pub instance_name: String,
}

/// Body for `POST …/instance/{instanceId}/reboot`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstanceReboot {
    pub r#type: RebootType,
}

impl OvhClient {
    /// `GET /cloud/project/{serviceName}/instance` — list instances.
    pub async fn cloud_instances(
        &self,
        service_name: &str,
        query: InstanceListQuery<'_>,
    ) -> Result<Vec<Instance>> {
        let mut path = project_path(service_name, self, "/instance");
        if let Some(region) = query.region {
            path = Self::append_query(&path, &[("region", region)]);
        }
        self.get_v1_url(&path).await
    }

    /// `POST /cloud/project/{serviceName}/instance` — create an instance.
    pub async fn cloud_instance_create(
        &self,
        service_name: &str,
        create: &InstanceCreate,
    ) -> Result<Instance> {
        let path = project_path(service_name, self, "/instance");
        self.post_v1(&path, create).await
    }

    /// `GET /cloud/project/{serviceName}/instance/{instanceId}` — instance details.
    pub async fn cloud_instance(&self, service_name: &str, instance_id: &str) -> Result<Instance> {
        let path = format!(
            "{}/{}",
            project_path(service_name, self, "/instance"),
            self.encode_segment(instance_id)
        );
        self.get_v1(&path).await
    }

    /// `PUT /cloud/project/{serviceName}/instance/{instanceId}` — rename instance.
    pub async fn cloud_instance_update(
        &self,
        service_name: &str,
        instance_id: &str,
        update: &InstanceUpdate,
    ) -> Result<()> {
        let path = format!(
            "{}/{}",
            project_path(service_name, self, "/instance"),
            self.encode_segment(instance_id)
        );
        self.put_v1(&path, update).await
    }

    /// `DELETE /cloud/project/{serviceName}/instance/{instanceId}`.
    pub async fn cloud_instance_delete(&self, service_name: &str, instance_id: &str) -> Result<()> {
        let path = format!(
            "{}/{}",
            project_path(service_name, self, "/instance"),
            self.encode_segment(instance_id)
        );
        self.delete_v1(&path).await
    }

    /// `POST …/instance/{instanceId}/start`.
    pub async fn cloud_instance_start(&self, service_name: &str, instance_id: &str) -> Result<()> {
        self.cloud_instance_action(service_name, instance_id, "start")
            .await
    }

    /// `POST …/instance/{instanceId}/stop`.
    pub async fn cloud_instance_stop(&self, service_name: &str, instance_id: &str) -> Result<()> {
        self.cloud_instance_action(service_name, instance_id, "stop")
            .await
    }

    /// `POST …/instance/{instanceId}/reboot`.
    pub async fn cloud_instance_reboot(
        &self,
        service_name: &str,
        instance_id: &str,
        reboot: InstanceReboot,
    ) -> Result<()> {
        let path = format!(
            "{}/{}/reboot",
            project_path(service_name, self, "/instance"),
            self.encode_segment(instance_id)
        );
        self.post_v1_void_body(&path, &reboot).await
    }

    /// `POST …/instance/{instanceId}/shelve`.
    pub async fn cloud_instance_shelve(&self, service_name: &str, instance_id: &str) -> Result<()> {
        self.cloud_instance_action(service_name, instance_id, "shelve")
            .await
    }

    /// `POST …/instance/{instanceId}/unshelve`.
    pub async fn cloud_instance_unshelve(
        &self,
        service_name: &str,
        instance_id: &str,
    ) -> Result<()> {
        self.cloud_instance_action(service_name, instance_id, "unshelve")
            .await
    }

    async fn cloud_instance_action(
        &self,
        service_name: &str,
        instance_id: &str,
        action: &str,
    ) -> Result<()> {
        let path = format!(
            "{}/{}/{action}",
            project_path(service_name, self, "/instance"),
            self.encode_segment(instance_id)
        );
        self.post_v1_void(&path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_instance_list_entry() {
        let inst: Instance = serde_json::from_str(
            r#"{
                "id": "i-1",
                "name": "web",
                "region": "GRA11",
                "flavorId": "f1",
                "addresses": [{"ip": "1.2.3.4", "version": 4}]
            }"#,
        )
        .unwrap();
        assert_eq!(inst.ip_addresses[0].ip, "1.2.3.4");
    }

    #[test]
    fn serializes_instance_create() {
        let body = InstanceCreate {
            name: "web-1".into(),
            region: "GRA11".into(),
            flavor_id: "abc".into(),
            image_id: Some("img".into()),
            ssh_key_id: None,
            availability_zone: None,
            group_id: None,
            monthly_billing: None,
            user_data: None,
            networks: None,
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains(r#""flavorId":"abc""#));
        assert!(json.contains(r#""name":"web-1""#));
    }
}
