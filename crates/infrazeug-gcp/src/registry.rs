use crate::client::GcpClientSource;
use crate::methods::{ensure_bucket, ensure_disk, ensure_instance, ensure_service_account_key};
use infrazeug_native::MethodRegistry;

pub fn method_registry(source: GcpClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    reg.register(ensure_instance(source.clone()));
    reg.register(ensure_disk(source.clone()));
    reg.register(ensure_bucket(source.clone()));
    reg.register(ensure_service_account_key(source));
    reg
}
