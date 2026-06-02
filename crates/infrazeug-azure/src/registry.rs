use crate::client::AzureClientSource;
use crate::methods::{ensure_container, ensure_disk, ensure_storage_key, ensure_vm};
use infrazeug_native::MethodRegistry;

pub fn method_registry(source: AzureClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    reg.register(ensure_vm(source.clone()));
    reg.register(ensure_disk(source.clone()));
    reg.register(ensure_container(source.clone()));
    reg.register(ensure_storage_key(source));
    reg
}
