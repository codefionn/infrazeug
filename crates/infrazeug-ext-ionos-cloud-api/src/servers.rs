//! Server management (`/datacenters/{id}/servers`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery, ResourceReference};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Server resource returned by the Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ElementMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<ServerProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Server properties (read model).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cores: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_cdrom: Option<ResourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_volume: Option<ResourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_family: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_features: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nic_multi_queue: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_network_bandwidth: Option<u32>,
}

/// Payload for creating a server.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ElementMetadata>,
    pub properties: ServerCreateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties accepted when creating a server.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerCreateProperties {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cores: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_cdrom: Option<ResourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_volume: Option<ResourceReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_family: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nic_multi_queue: Option<bool>,
}

/// Payload for updating a server.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerUpdate {
    pub properties: ServerUpdateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties accepted when updating a server.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cores: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_family: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nic_multi_queue: Option<bool>,
}

fn server_path(client: &IonosClient, datacenter_id: &str, server_id: Option<&str>) -> String {
    let mut path = format!("/datacenters/{}/servers", client.encode_path(datacenter_id));
    if let Some(server_id) = server_id {
        path.push('/');
        path.push_str(&client.encode_path(server_id));
    }
    path
}

impl IonosClient {
    /// `GET /datacenters/{dc}/servers` — list servers in a data center.
    pub async fn servers(
        &self,
        datacenter_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<Server>> {
        self.get(&server_path(self, datacenter_id, None), query)
            .await
    }

    /// `GET /datacenters/{dc}/servers/{id}` — retrieve one server.
    pub async fn server(
        &self,
        datacenter_id: &str,
        server_id: &str,
        query: &ListQuery,
    ) -> Result<Server> {
        self.get(&server_path(self, datacenter_id, Some(server_id)), query)
            .await
    }

    /// `POST /datacenters/{dc}/servers` — create a server.
    pub async fn create_server(
        &self,
        datacenter_id: &str,
        body: &ServerCreate,
        query: &ListQuery,
    ) -> Result<Server> {
        self.post_json(&server_path(self, datacenter_id, None), body, query)
            .await
    }

    /// `PUT /datacenters/{dc}/servers/{id}` — replace a server.
    pub async fn update_server(
        &self,
        datacenter_id: &str,
        server_id: &str,
        body: &ServerUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<Server> {
        self.put_json(
            &server_path(self, datacenter_id, Some(server_id)),
            body,
            query,
            etag,
        )
        .await
    }

    /// `DELETE /datacenters/{dc}/servers/{id}` — delete a server.
    pub async fn delete_server(
        &self,
        datacenter_id: &str,
        server_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &server_path(self, datacenter_id, Some(server_id)),
            query,
            etag,
        )
        .await
    }

    /// `POST /datacenters/{dc}/servers/{id}/reboot` — reboot a server.
    pub async fn reboot_server(&self, datacenter_id: &str, server_id: &str) -> Result<()> {
        self.post(
            &format!(
                "{}/reboot",
                server_path(self, datacenter_id, Some(server_id))
            ),
            &serde_json::json!({}),
            &ListQuery::default(),
        )
        .await
    }

    /// `POST /datacenters/{dc}/servers/{id}/start` — start an enterprise server.
    pub async fn start_server(&self, datacenter_id: &str, server_id: &str) -> Result<()> {
        self.post(
            &format!(
                "{}/start",
                server_path(self, datacenter_id, Some(server_id))
            ),
            &serde_json::json!({}),
            &ListQuery::default(),
        )
        .await
    }

    /// `POST /datacenters/{dc}/servers/{id}/stop` — stop an enterprise server.
    pub async fn stop_server(&self, datacenter_id: &str, server_id: &str) -> Result<()> {
        self.post(
            &format!("{}/stop", server_path(self, datacenter_id, Some(server_id))),
            &serde_json::json!({}),
            &ListQuery::default(),
        )
        .await
    }

    /// `POST /datacenters/{dc}/servers/{id}/suspend` — suspend a Cube server.
    pub async fn suspend_server(&self, datacenter_id: &str, server_id: &str) -> Result<()> {
        self.post(
            &format!(
                "{}/suspend",
                server_path(self, datacenter_id, Some(server_id))
            ),
            &serde_json::json!({}),
            &ListQuery::default(),
        )
        .await
    }

    /// `POST /datacenters/{dc}/servers/{id}/resume` — resume a Cube server.
    pub async fn resume_server(&self, datacenter_id: &str, server_id: &str) -> Result<()> {
        self.post(
            &format!(
                "{}/resume",
                server_path(self, datacenter_id, Some(server_id))
            ),
            &serde_json::json!({}),
            &ListQuery::default(),
        )
        .await
    }

    /// `GET /datacenters/{dc}/servers/{srv}/volumes` — list volumes attached to a server.
    pub async fn server_volumes(
        &self,
        datacenter_id: &str,
        server_id: &str,
        query: &ListQuery,
    ) -> Result<crate::types::Collection<crate::volumes::Volume>> {
        self.get(
            &format!(
                "{}/volumes",
                server_path(self, datacenter_id, Some(server_id))
            ),
            query,
        )
        .await
    }

    /// `POST /datacenters/{dc}/servers/{srv}/volumes` — attach a volume to a server.
    pub async fn attach_server_volume(
        &self,
        datacenter_id: &str,
        server_id: &str,
        volume_id: &str,
        query: &ListQuery,
    ) -> Result<crate::volumes::Volume> {
        let body = serde_json::json!({
            "id": volume_id,
            "type": "volume"
        });
        self.post_json(
            &format!(
                "{}/volumes",
                server_path(self, datacenter_id, Some(server_id))
            ),
            &body,
            query,
        )
        .await
    }

    /// `DELETE /datacenters/{dc}/servers/{srv}/volumes/{vol}` — detach a volume.
    pub async fn detach_server_volume(
        &self,
        datacenter_id: &str,
        server_id: &str,
        volume_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &format!(
                "{}/volumes/{}",
                server_path(self, datacenter_id, Some(server_id)),
                self.encode_path(volume_id)
            ),
            query,
            etag,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_server() {
        let json = r#"{
            "id":"srv-1",
            "type":"server",
            "properties":{"name":"web","cores":2,"ram":4096,"vmState":"RUNNING"}
        }"#;
        let srv: Server = serde_json::from_str(json).unwrap();
        assert_eq!(srv.properties.as_ref().unwrap().cores, Some(2));
    }
}
