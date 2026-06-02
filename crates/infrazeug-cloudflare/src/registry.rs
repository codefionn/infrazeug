//! Method registry for custom agent binaries linking Cloudflare nodes.

use crate::client::CloudflareClientSource;
use crate::methods::{
    ensure_dns_record, ensure_firewall_access_rule, ensure_kv_namespace, ensure_r2_bucket,
    ensure_waf_custom_rule, ensure_zone_setting,
};
use infrazeug_native::MethodRegistry;

/// Register all Cloudflare tier-1 methods for a shared [`CloudflareClientSource`].
pub fn method_registry(source: CloudflareClientSource) -> MethodRegistry {
    let mut reg = MethodRegistry::new();
    let source_dns = source.clone();
    let source_setting = source.clone();
    let source_access = source.clone();
    let source_waf = source.clone();
    let source_r2 = source.clone();
    reg.register(ensure_dns_record(source_dns));
    reg.register(ensure_zone_setting(source_setting));
    reg.register(ensure_firewall_access_rule(source_access));
    reg.register(ensure_waf_custom_rule(source_waf));
    reg.register(ensure_r2_bucket(source_r2));
    reg.register(ensure_kv_namespace(source));
    reg
}
