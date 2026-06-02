//! Managed Kubernetes (`/k8s`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ElementMetadata, ListQuery};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Kubernetes cluster resource.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesCluster {
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
    pub properties: Option<KubernetesClusterProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Kubernetes cluster properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesClusterProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k8s_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance_window: Option<MaintenanceWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nat_gateway_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_subnet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_subnet_allow_list: Option<Vec<String>>,
}

/// Maintenance window schedule.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceWindow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day_of_the_week: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
}

/// Payload for creating a Kubernetes cluster.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesClusterCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ElementMetadata>,
    pub properties: KubernetesClusterCreateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties for creating a cluster.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesClusterCreateProperties {
    pub name: String,
    pub k8s_version: String,
    pub location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance_window: Option<MaintenanceWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_subnet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_subnet_allow_list: Option<Vec<String>>,
}

/// Payload for updating a Kubernetes cluster.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesClusterUpdate {
    pub properties: KubernetesClusterUpdateProperties,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entities: Option<HashMap<String, serde_json::Value>>,
}

/// Properties for updating a cluster.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KubernetesClusterUpdateProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k8s_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance_window: Option<MaintenanceWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_subnet_allow_list: Option<Vec<String>>,
}

/// Node pool resource.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodePool {
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
    pub properties: Option<NodePoolProperties>,
}

/// Node pool properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodePoolProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datacenter_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cores_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k8s_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_ips: Option<Vec<String>>,
}

/// Payload for creating a node pool.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodePoolCreate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ElementMetadata>,
    pub properties: NodePoolCreateProperties,
}

/// Properties for creating a node pool.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodePoolCreateProperties {
    pub name: String,
    pub datacenter_id: String,
    pub node_count: u32,
    pub server_type: String,
    pub cores_count: u32,
    pub ram_size: u32,
    pub storage_type: String,
    pub storage_size: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k8s_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_ips: Option<Vec<String>>,
}

/// Kubeconfig response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kubeconfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<KubeconfigProperties>,
}

/// Kubeconfig properties.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KubeconfigProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kubeconfig: Option<String>,
}

fn cluster_path(client: &IonosClient, cluster_id: Option<&str>) -> String {
    let mut path = "/k8s".to_string();
    if let Some(cluster_id) = cluster_id {
        path.push('/');
        path.push_str(&client.encode_path(cluster_id));
    }
    path
}

fn nodepool_path(client: &IonosClient, cluster_id: &str, pool_id: Option<&str>) -> String {
    let mut path = format!("{}/nodepools", cluster_path(client, Some(cluster_id)));
    if let Some(pool_id) = pool_id {
        path.push('/');
        path.push_str(&client.encode_path(pool_id));
    }
    path
}

impl IonosClient {
    /// `GET /k8s` — list Kubernetes clusters.
    pub async fn kubernetes_clusters(
        &self,
        query: &ListQuery,
    ) -> Result<Collection<KubernetesCluster>> {
        self.get("/k8s", query).await
    }

    /// `GET /k8s/{id}` — retrieve one cluster.
    pub async fn kubernetes_cluster(
        &self,
        cluster_id: &str,
        query: &ListQuery,
    ) -> Result<KubernetesCluster> {
        self.get(&cluster_path(self, Some(cluster_id)), query).await
    }

    /// `POST /k8s` — create a Kubernetes cluster.
    pub async fn create_kubernetes_cluster(
        &self,
        body: &KubernetesClusterCreate,
        query: &ListQuery,
    ) -> Result<KubernetesCluster> {
        self.post_json("/k8s", body, query).await
    }

    /// `PUT /k8s/{id}` — update a Kubernetes cluster.
    pub async fn update_kubernetes_cluster(
        &self,
        cluster_id: &str,
        body: &KubernetesClusterUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<KubernetesCluster> {
        self.put_json(&cluster_path(self, Some(cluster_id)), body, query, etag)
            .await
    }

    /// `DELETE /k8s/{id}` — delete a Kubernetes cluster.
    pub async fn delete_kubernetes_cluster(
        &self,
        cluster_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(&cluster_path(self, Some(cluster_id)), query, etag)
            .await
    }

    /// `GET /k8s/{id}/kubeconfig` — retrieve cluster kubeconfig.
    pub async fn kubernetes_kubeconfig(
        &self,
        cluster_id: &str,
        query: &ListQuery,
    ) -> Result<Kubeconfig> {
        self.get(
            &format!("{}/kubeconfig", cluster_path(self, Some(cluster_id))),
            query,
        )
        .await
    }

    /// `GET /k8s/{id}/nodepools` — list node pools.
    pub async fn node_pools(
        &self,
        cluster_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<NodePool>> {
        self.get(&nodepool_path(self, cluster_id, None), query)
            .await
    }

    /// `GET /k8s/{id}/nodepools/{poolId}` — retrieve one node pool.
    pub async fn node_pool(
        &self,
        cluster_id: &str,
        pool_id: &str,
        query: &ListQuery,
    ) -> Result<NodePool> {
        self.get(&nodepool_path(self, cluster_id, Some(pool_id)), query)
            .await
    }

    /// `POST /k8s/{id}/nodepools` — create a node pool.
    pub async fn create_node_pool(
        &self,
        cluster_id: &str,
        body: &NodePoolCreate,
        query: &ListQuery,
    ) -> Result<NodePool> {
        self.post_json(&nodepool_path(self, cluster_id, None), body, query)
            .await
    }

    /// `DELETE /k8s/{id}/nodepools/{poolId}` — delete a node pool.
    pub async fn delete_node_pool(
        &self,
        cluster_id: &str,
        pool_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(&nodepool_path(self, cluster_id, Some(pool_id)), query, etag)
            .await
    }

    /// `GET /k8s/versions` — list supported Kubernetes versions.
    pub async fn kubernetes_versions(&self, query: &ListQuery) -> Result<serde_json::Value> {
        self.get("/k8s/versions", query).await
    }
}
