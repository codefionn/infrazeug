//! Data center management (`/datacenters`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Data center resource returned by the Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Datacenter {
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
    pub properties: Option<DatacenterProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Data center properties (read model).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DatacenterProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sec_auth_protection: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6_cidr_block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_security_group_id: Option<String>,
}

/// Payload for creating a data center.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DatacenterCreate {
    pub properties: DatacenterCreateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties accepted when creating a data center.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DatacenterCreateProperties {
    pub name: String,
    pub location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sec_auth_protection: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_default_security_group: Option<bool>,
}

/// Payload for updating a data center.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DatacenterUpdate {
    pub properties: DatacenterUpdateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties accepted when updating a data center.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DatacenterUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sec_auth_protection: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_security_group_id: Option<String>,
}

impl IonosClient {
    /// `GET /datacenters` — list data centers.
    pub async fn datacenters(&self, query: &ListQuery) -> Result<Collection<Datacenter>> {
        self.get("/datacenters", query).await
    }

    /// `GET /datacenters/{id}` — retrieve one data center.
    pub async fn datacenter(&self, id: &str, query: &ListQuery) -> Result<Datacenter> {
        self.get(&format!("/datacenters/{}", self.encode_path(id)), query)
            .await
    }

    /// `POST /datacenters` — create a data center.
    pub async fn create_datacenter(
        &self,
        body: &DatacenterCreate,
        query: &ListQuery,
    ) -> Result<Datacenter> {
        self.post_json("/datacenters", body, query).await
    }

    /// `PUT /datacenters/{id}` — replace a data center.
    pub async fn update_datacenter(
        &self,
        id: &str,
        body: &DatacenterUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<Datacenter> {
        self.put_json(
            &format!("/datacenters/{}", self.encode_path(id)),
            body,
            query,
            etag,
        )
        .await
    }

    /// `DELETE /datacenters/{id}` — delete a data center.
    pub async fn delete_datacenter(
        &self,
        id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &format!("/datacenters/{}", self.encode_path(id)),
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
    fn deserialize_datacenter() {
        let json = r#"{
            "id":"dc-1",
            "type":"datacenter",
            "properties":{"name":"prod","location":"de/fra"}
        }"#;
        let dc: Datacenter = serde_json::from_str(json).unwrap();
        assert_eq!(dc.id.as_deref(), Some("dc-1"));
        assert_eq!(
            dc.properties.as_ref().unwrap().location.as_deref(),
            Some("de/fra")
        );
    }

    #[test]
    fn serialize_create_skips_none() {
        let body = DatacenterCreate {
            properties: DatacenterCreateProperties {
                name: "prod".into(),
                location: "de/fra".into(),
                description: None,
                sec_auth_protection: None,
                create_default_security_group: None,
            },
            entities: None,
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("\"name\":\"prod\""));
        assert!(!json.contains("description"));
    }
}
