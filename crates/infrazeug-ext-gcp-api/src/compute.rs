//! Compute Engine instances and disks.

use crate::client::{api_error, GcpClient};
use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Instance {
    pub instance_id: String,
    pub name: String,
    pub zone: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub internal_ip: Option<String>,
    #[serde(default)]
    pub external_ip: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Disk {
    pub disk_id: String,
    pub name: String,
    pub zone: String,
    pub size_gb: u32,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct InstanceCreate {
    pub name: String,
    pub zone: String,
    pub machine_type: String,
    pub source_image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_size_gb: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiskCreate {
    pub name: String,
    pub zone: String,
    pub size_gb: u32,
    pub disk_type: String,
}

#[derive(Debug, Deserialize)]
struct InstanceList {
    #[serde(default)]
    items: Vec<InstanceResource>,
}

#[derive(Debug, Deserialize)]
struct InstanceResource {
    id: Option<String>,
    name: Option<String>,
    zone: Option<String>,
    status: Option<String>,
    #[serde(default)]
    network_interfaces: Vec<NetworkInterface>,
}

#[derive(Debug, Deserialize)]
struct NetworkInterface {
    #[serde(default)]
    network_ip: Option<String>,
    #[serde(default)]
    access_configs: Vec<AccessConfig>,
}

#[derive(Debug, Deserialize)]
struct AccessConfig {
    #[serde(default)]
    nat_ip: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiskList {
    #[serde(default)]
    items: Vec<DiskResource>,
}

#[derive(Debug, Deserialize)]
struct DiskResource {
    id: Option<String>,
    name: Option<String>,
    zone: Option<String>,
    size_gb: Option<String>,
}

fn zone_name(zone_url: &str) -> String {
    zone_url.rsplit('/').next().unwrap_or(zone_url).to_string()
}

fn to_instance(item: InstanceResource) -> Option<Instance> {
    Some(Instance {
        instance_id: item.id?,
        name: item.name?,
        zone: item.zone.as_ref().map(|z| zone_name(z)).unwrap_or_default(),
        status: item.status,
        internal_ip: item
            .network_interfaces
            .first()
            .and_then(|ni| ni.network_ip.clone()),
        external_ip: item
            .network_interfaces
            .first()
            .and_then(|ni| ni.access_configs.first())
            .and_then(|ac| ac.nat_ip.clone()),
    })
}

fn to_disk(item: DiskResource) -> Option<Disk> {
    Some(Disk {
        disk_id: item.id?,
        name: item.name?,
        zone: item.zone.as_ref().map(|z| zone_name(z)).unwrap_or_default(),
        size_gb: item.size_gb.and_then(|s| s.parse().ok()).unwrap_or(0),
    })
}

impl GcpClient {
    pub async fn compute_instance(&self, zone: &str, name: &str) -> Result<Option<Instance>> {
        let url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/zones/{}/instances/{}",
            self.project_id(),
            zone,
            name
        );
        match self.get::<InstanceResource>(&url).await {
            Ok(item) => Ok(to_instance(item)),
            Err(crate::error::GcpError::Api { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn compute_instances(&self, zone: &str) -> Result<Vec<Instance>> {
        let url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/zones/{}/instances",
            self.project_id(),
            zone
        );
        let list: InstanceList = self.get(&url).await?;
        Ok(list.items.into_iter().filter_map(to_instance).collect())
    }

    pub async fn compute_instance_create(&self, create: &InstanceCreate) -> Result<Instance> {
        let machine_type = format!("zones/{}/machineTypes/{}", create.zone, create.machine_type);
        let disk_size = create.disk_size_gb.unwrap_or(10);
        let body = serde_json::json!({
            "name": create.name,
            "machineType": format!("https://compute.googleapis.com/compute/v1/projects/{}/{}", self.project_id(), machine_type),
            "disks": [{
                "boot": true,
                "autoDelete": true,
                "initializeParams": {
                    "sourceImage": create.source_image,
                    "diskSizeGb": disk_size
                }
            }],
            "networkInterfaces": [{
                "network": "global/networks/default",
                "accessConfigs": [{
                    "name": "External NAT",
                    "type": "ONE_TO_ONE_NAT"
                }]
            }]
        });
        let url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/zones/{}/instances",
            self.project_id(),
            create.zone
        );
        let item: InstanceResource = self.post_json(&url, &body).await?;
        to_instance(item).ok_or_else(|| {
            api_error(
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                "instance has no id",
            )
        })
    }

    pub async fn compute_disk(&self, zone: &str, name: &str) -> Result<Option<Disk>> {
        let url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/zones/{}/disks/{}",
            self.project_id(),
            zone,
            name
        );
        match self.get::<DiskResource>(&url).await {
            Ok(item) => Ok(to_disk(item)),
            Err(crate::error::GcpError::Api { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn compute_disks(&self, zone: &str) -> Result<Vec<Disk>> {
        let url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/zones/{}/disks",
            self.project_id(),
            zone
        );
        let list: DiskList = self.get(&url).await?;
        Ok(list.items.into_iter().filter_map(to_disk).collect())
    }

    pub async fn compute_disk_create(&self, create: &DiskCreate) -> Result<Disk> {
        let body = serde_json::json!({
            "name": create.name,
            "sizeGb": create.size_gb.to_string(),
            "type": format!("zones/{}/diskTypes/{}", create.zone, create.disk_type),
        });
        let url = format!(
            "https://compute.googleapis.com/compute/v1/projects/{}/zones/{}/disks",
            self.project_id(),
            create.zone
        );
        let item: DiskResource = self.post_json(&url, &body).await?;
        to_disk(item)
            .ok_or_else(|| api_error(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "disk has no id"))
    }
}
