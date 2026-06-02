//! Method registry for custom agent binaries linking UniFi nodes.

use crate::client::UnifiClientSource;
use crate::methods::{
    ensure_dns_record, ensure_firewall_group, ensure_firewall_rule, ensure_fixed_ip,
    ensure_network, ensure_port_forward, ensure_user_group, ensure_wlan,
};
use infrazeug_native::MethodRegistry;

/// Register all UniFi tier-1 methods for a shared [`UnifiClientSource`].
pub fn method_registry(source: UnifiClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    reg.register(ensure_network(source.clone()));
    reg.register(ensure_port_forward(source.clone()));
    reg.register(ensure_wlan(source.clone()));
    reg.register(ensure_dns_record(source.clone()));
    reg.register(ensure_firewall_group(source.clone()));
    reg.register(ensure_firewall_rule(source.clone()));
    reg.register(ensure_user_group(source.clone()));
    reg.register(ensure_fixed_ip(source));
    reg
}
