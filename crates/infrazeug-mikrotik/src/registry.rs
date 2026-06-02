//! Method registry for custom agent binaries linking MikroTik nodes.

use crate::client::MikrotikClientSource;
use crate::methods::{ensure_firewall_rule, ensure_ip_address};
use infrazeug_native::MethodRegistry;

/// Register all MikroTik tier-1 methods for a shared [`MikrotikClientSource`].
pub fn method_registry(source: MikrotikClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    reg.register(ensure_ip_address(source.clone()));
    reg.register(ensure_firewall_rule(source));
    reg
}
