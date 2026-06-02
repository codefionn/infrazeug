//! Azure tier-1 native nodes for infrazeug playbooks.

mod builder;
mod client;
mod methods;
mod registry;

pub use builder::{AzureInfraBuilder, AzureInfraExt};
pub use client::{client_from_env, AzureClientSource};
pub use methods::{
    ensure_container, ensure_disk, ensure_storage_key, ensure_vm, EnsureContainer,
    EnsureContainerInput, EnsureContainerOutput, EnsureDisk, EnsureDiskInput, EnsureDiskOutput,
    EnsureStorageKey, EnsureStorageKeyInput, EnsureStorageKeyOutput, EnsureVm, EnsureVmInput,
    EnsureVmOutput, ENSURE_CONTAINER, ENSURE_DISK, ENSURE_STORAGE_KEY, ENSURE_VM,
};
pub use registry::method_registry;

pub use infrazeug_ext_azure_api::{AzureClient, AzureConfig, AzureCredentials};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
