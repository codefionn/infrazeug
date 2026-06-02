use crate::client::AzureClientSource;
use async_trait::async_trait;
use infrazeug_ext_azure_api::compute::{ImageReference, VirtualMachine, VirtualMachineCreate};
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_VM: &str = "azure.ensure_vm";

pub type EnsureVm = EnsureResource<VmResource>;

pub fn ensure_vm(source: AzureClientSource) -> EnsureVm {
    EnsureResource::new(VmResource::new(source))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureVmInput {
    pub name: String,
    pub resource_group: String,
    pub location: String,
    pub vm_size: String,
    pub admin_username: String,
    pub admin_password: String,
    pub image_publisher: String,
    pub image_offer: String,
    pub image_sku: String,
    #[serde(default = "default_image_version")]
    pub image_version: String,
}

fn default_image_version() -> String {
    "latest".into()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureVmOutput {
    pub vm_id: String,
    pub name: String,
    pub resource_group: String,
    pub location: String,
}

#[derive(Clone)]
pub struct VmResource {
    source: AzureClientSource,
}

impl VmResource {
    pub fn new(source: AzureClientSource) -> Self {
        Self { source }
    }
}

fn to_output(vm: VirtualMachine) -> EnsureVmOutput {
    EnsureVmOutput {
        vm_id: vm.vm_id,
        name: vm.name,
        resource_group: vm.resource_group,
        location: vm.location,
    }
}

#[async_trait]
impl Resource for VmResource {
    type Spec = EnsureVmInput;
    type State = EnsureVmOutput;

    fn kind(&self) -> &'static str {
        ENSURE_VM
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let vms = client
            .compute_virtual_machines(&spec.resource_group)
            .await
            .map_err(ResourceError::provider)?;
        Ok(vms.into_iter().find(|v| v.name == spec.name).map(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let created = client
            .compute_virtual_machine_create(&VirtualMachineCreate {
                name: spec.name.clone(),
                resource_group: spec.resource_group.clone(),
                location: spec.location.clone(),
                vm_size: spec.vm_size.clone(),
                admin_username: spec.admin_username.clone(),
                admin_password: spec.admin_password.clone(),
                image_reference: ImageReference {
                    publisher: spec.image_publisher.clone(),
                    offer: spec.image_offer.clone(),
                    sku: spec.image_sku.clone(),
                    version: spec.image_version.clone(),
                },
            })
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(created))
    }
}
