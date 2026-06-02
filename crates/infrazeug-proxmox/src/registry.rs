//! Method registry for custom agent binaries linking Proxmox nodes.

use crate::client::ProxmoxClientSource;
use crate::methods::{ensure_lxc, ensure_qemu};
use infrazeug_native::MethodRegistry;

/// Register all Proxmox tier-1 methods for a shared [`ProxmoxClientSource`].
pub fn method_registry(source: ProxmoxClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    reg.register(ensure_qemu(source.clone()));
    reg.register(ensure_lxc(source));
    reg
}
