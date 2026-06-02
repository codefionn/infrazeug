use crate::client::{api_error, AzureClient, ARM_API_VERSION};
use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct VirtualMachine {
    pub vm_id: String,
    pub name: String,
    pub resource_group: String,
    pub location: String,
    #[serde(default)]
    pub private_ip: Option<String>,
    #[serde(default)]
    pub public_ip: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagedDisk {
    pub disk_id: String,
    pub name: String,
    pub resource_group: String,
    pub location: String,
    pub size_gb: u32,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct VirtualMachineCreate {
    pub name: String,
    pub resource_group: String,
    pub location: String,
    pub vm_size: String,
    pub admin_username: String,
    pub admin_password: String,
    pub image_reference: ImageReference,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImageReference {
    pub publisher: String,
    pub offer: String,
    pub sku: String,
    pub version: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ManagedDiskCreate {
    pub name: String,
    pub resource_group: String,
    pub location: String,
    pub size_gb: u32,
    pub sku: String,
}

#[derive(Debug, Deserialize)]
struct VmList {
    #[serde(default)]
    value: Vec<VmResource>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct VmResource {
    #[allow(dead_code)]
    id: Option<String>,
    #[allow(dead_code)]
    name: Option<String>,
    #[allow(dead_code)]
    location: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    properties: Option<VmProperties>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct VmProperties {
    #[serde(default)]
    storage_profile: Option<StorageProfile>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StorageProfile {
    #[serde(default)]
    os_disk: Option<OsDisk>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OsDisk {
    #[serde(default)]
    managed_disk: Option<ManagedDiskRef>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ManagedDiskRef {
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiskList {
    #[serde(default)]
    value: Vec<DiskResource>,
}

#[derive(Debug, Deserialize)]
struct DiskResource {
    id: Option<String>,
    name: Option<String>,
    location: Option<String>,
    #[serde(default)]
    properties: Option<DiskProperties>,
}

#[derive(Debug, Deserialize)]
struct DiskProperties {
    #[serde(default)]
    disk_size_gb: Option<u32>,
}

fn resource_group_from_id(id: &str) -> String {
    id.split("/resourceGroups/")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .unwrap_or_default()
        .to_string()
}

impl AzureClient {
    pub async fn compute_virtual_machine(
        &self,
        resource_group: &str,
        name: &str,
    ) -> Result<Option<VirtualMachine>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Compute/virtualMachines/{}?api-version={ARM_API_VERSION}",
            self.subscription_id(),
            resource_group,
            name
        );
        match self.arm_get::<VmResource>(&url).await {
            Ok(item) => {
                let id = item.id.ok_or_else(|| {
                    api_error(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "vm has no id")
                })?;
                Ok(Some(VirtualMachine {
                    vm_id: id.clone(),
                    name: item.name.unwrap_or_else(|| name.into()),
                    resource_group: resource_group_from_id(&id),
                    location: item.location.unwrap_or_default(),
                    private_ip: None,
                    public_ip: None,
                }))
            }
            Err(crate::error::AzureError::Api { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn compute_virtual_machines(
        &self,
        resource_group: &str,
    ) -> Result<Vec<VirtualMachine>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Compute/virtualMachines?api-version={ARM_API_VERSION}",
            self.subscription_id(),
            resource_group
        );
        let list: VmList = self.arm_get(&url).await?;
        Ok(list
            .value
            .into_iter()
            .filter_map(|item| {
                let id = item.id?;
                Some(VirtualMachine {
                    vm_id: id.clone(),
                    name: item.name?,
                    resource_group: resource_group_from_id(&id),
                    location: item.location.unwrap_or_default(),
                    private_ip: None,
                    public_ip: None,
                })
            })
            .collect())
    }

    pub async fn compute_virtual_machine_create(
        &self,
        create: &VirtualMachineCreate,
    ) -> Result<VirtualMachine> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Compute/virtualMachines/{}?api-version={ARM_API_VERSION}",
            self.subscription_id(),
            create.resource_group,
            create.name
        );
        let body = serde_json::json!({
            "location": create.location,
            "properties": {
                "hardwareProfile": { "vmSize": create.vm_size },
                "storageProfile": {
                    "imageReference": {
                        "publisher": create.image_reference.publisher,
                        "offer": create.image_reference.offer,
                        "sku": create.image_reference.sku,
                        "version": create.image_reference.version
                    },
                    "osDisk": {
                        "createOption": "FromImage",
                        "managedDisk": { "storageAccountType": "Standard_LRS" }
                    }
                },
                "osProfile": {
                    "computerName": create.name,
                    "adminUsername": create.admin_username,
                    "adminPassword": create.admin_password
                },
                "networkProfile": {
                    "networkInterfaces": [{
                        "id": format!(
                            "/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Network/networkInterfaces/{}-nic",
                            self.subscription_id(), create.resource_group, create.name
                        ),
                        "properties": { "primary": true }
                    }]
                }
            }
        });
        let item: VmResource = self.arm_put(&url, &body).await?;
        let id = item
            .id
            .ok_or_else(|| api_error(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "vm has no id"))?;
        Ok(VirtualMachine {
            vm_id: id.clone(),
            name: item.name.unwrap_or_else(|| create.name.clone()),
            resource_group: resource_group_from_id(&id),
            location: item.location.unwrap_or_else(|| create.location.clone()),
            private_ip: None,
            public_ip: None,
        })
    }

    pub async fn compute_managed_disk(
        &self,
        resource_group: &str,
        name: &str,
    ) -> Result<Option<ManagedDisk>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Compute/disks/{}?api-version={ARM_API_VERSION}",
            self.subscription_id(),
            resource_group,
            name
        );
        match self.arm_get::<DiskResource>(&url).await {
            Ok(item) => {
                let id = item.id.ok_or_else(|| {
                    api_error(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "disk has no id")
                })?;
                Ok(Some(ManagedDisk {
                    disk_id: id.clone(),
                    name: item.name.unwrap_or_else(|| name.into()),
                    resource_group: resource_group_from_id(&id),
                    location: item.location.unwrap_or_default(),
                    size_gb: item.properties.and_then(|p| p.disk_size_gb).unwrap_or(0),
                }))
            }
            Err(crate::error::AzureError::Api { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn compute_managed_disks(&self, resource_group: &str) -> Result<Vec<ManagedDisk>> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Compute/disks?api-version={ARM_API_VERSION}",
            self.subscription_id(),
            resource_group
        );
        let list: DiskList = self.arm_get(&url).await?;
        Ok(list
            .value
            .into_iter()
            .filter_map(|item| {
                let id = item.id?;
                Some(ManagedDisk {
                    disk_id: id.clone(),
                    name: item.name?,
                    resource_group: resource_group_from_id(&id),
                    location: item.location.unwrap_or_default(),
                    size_gb: item.properties.and_then(|p| p.disk_size_gb).unwrap_or(0),
                })
            })
            .collect())
    }

    pub async fn compute_managed_disk_create(
        &self,
        create: &ManagedDiskCreate,
    ) -> Result<ManagedDisk> {
        let url = format!(
            "https://management.azure.com/subscriptions/{}/resourceGroups/{}/providers/Microsoft.Compute/disks/{}?api-version={ARM_API_VERSION}",
            self.subscription_id(),
            create.resource_group,
            create.name
        );
        let body = serde_json::json!({
            "location": create.location,
            "properties": {
                "creationData": { "createOption": "Empty" },
                "diskSizeGB": create.size_gb,
            },
            "sku": { "name": create.sku }
        });
        let item: DiskResource = self.arm_put(&url, &body).await?;
        let id = item.id.ok_or_else(|| {
            api_error(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "disk has no id")
        })?;
        Ok(ManagedDisk {
            disk_id: id.clone(),
            name: item.name.unwrap_or_else(|| create.name.clone()),
            resource_group: resource_group_from_id(&id),
            location: item.location.unwrap_or_else(|| create.location.clone()),
            size_gb: create.size_gb,
        })
    }
}
