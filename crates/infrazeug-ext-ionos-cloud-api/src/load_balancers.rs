//! Load balancer management (`/datacenters/{dc}/loadbalancers`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Load balancer resource.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoadBalancer {
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
    pub properties: Option<LoadBalancerProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Load balancer properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoadBalancerProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<bool>,
}

/// Payload for creating a load balancer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoadBalancerCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    pub properties: LoadBalancerCreateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties for creating a load balancer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoadBalancerCreateProperties {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<bool>,
}

/// Payload for updating a load balancer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoadBalancerUpdate {
    pub properties: LoadBalancerUpdateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties for updating a load balancer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LoadBalancerUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp: Option<bool>,
}

fn load_balancer_path(client: &IonosClient, datacenter_id: &str, lb_id: Option<&str>) -> String {
    let mut path = format!(
        "/datacenters/{}/loadbalancers",
        client.encode_path(datacenter_id)
    );
    if let Some(lb_id) = lb_id {
        path.push('/');
        path.push_str(&client.encode_path(lb_id));
    }
    path
}

impl IonosClient {
    /// `GET /datacenters/{dc}/loadbalancers` — list load balancers.
    pub async fn load_balancers(
        &self,
        datacenter_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<LoadBalancer>> {
        self.get(&load_balancer_path(self, datacenter_id, None), query)
            .await
    }

    /// Retrieve one load balancer.
    pub async fn load_balancer(
        &self,
        datacenter_id: &str,
        lb_id: &str,
        query: &ListQuery,
    ) -> Result<LoadBalancer> {
        self.get(&load_balancer_path(self, datacenter_id, Some(lb_id)), query)
            .await
    }

    /// Create a load balancer.
    pub async fn create_load_balancer(
        &self,
        datacenter_id: &str,
        body: &LoadBalancerCreate,
        query: &ListQuery,
    ) -> Result<LoadBalancer> {
        self.post_json(&load_balancer_path(self, datacenter_id, None), body, query)
            .await
    }

    /// Update a load balancer.
    pub async fn update_load_balancer(
        &self,
        datacenter_id: &str,
        lb_id: &str,
        body: &LoadBalancerUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<LoadBalancer> {
        self.put_json(
            &load_balancer_path(self, datacenter_id, Some(lb_id)),
            body,
            query,
            etag,
        )
        .await
    }

    /// Delete a load balancer.
    pub async fn delete_load_balancer(
        &self,
        datacenter_id: &str,
        lb_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &load_balancer_path(self, datacenter_id, Some(lb_id)),
            query,
            etag,
        )
        .await
    }
}
