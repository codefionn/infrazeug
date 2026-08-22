//! Tier-1 resource methods for Cloudflare.

mod account;
mod dns_record;
mod dns_record_absent;
mod firewall_access_rule;
mod kv_namespace;
mod r2_bucket;
mod waf_custom_rule;
mod zone;
mod zone_setting;

pub use dns_record::{
    ensure_dns_record, EnsureDnsRecord, EnsureDnsRecordInput, EnsureDnsRecordOutput,
    ENSURE_DNS_RECORD,
};
pub use dns_record_absent::{
    ensure_dns_record_absent, EnsureDnsRecordAbsent, EnsureDnsRecordAbsentInput,
    EnsureDnsRecordAbsentOutput, ENSURE_DNS_RECORD_ABSENT,
};
pub use firewall_access_rule::{
    ensure_firewall_access_rule, EnsureFirewallAccessRule, EnsureFirewallAccessRuleInput,
    EnsureFirewallAccessRuleOutput, ENSURE_FIREWALL_ACCESS_RULE,
};
pub use kv_namespace::{
    ensure_kv_namespace, EnsureKvNamespace, EnsureKvNamespaceInput, EnsureKvNamespaceOutput,
    ENSURE_KV_NAMESPACE,
};
pub use r2_bucket::{
    ensure_r2_bucket, EnsureR2Bucket, EnsureR2BucketInput, EnsureR2BucketOutput, ENSURE_R2_BUCKET,
};
pub use waf_custom_rule::{
    ensure_waf_custom_rule, EnsureWafCustomRule, EnsureWafCustomRuleInput,
    EnsureWafCustomRuleOutput, ENSURE_WAF_CUSTOM_RULE,
};
pub use zone_setting::{
    ensure_zone_setting, EnsureZoneSetting, EnsureZoneSettingInput, EnsureZoneSettingOutput,
    ENSURE_ZONE_SETTING,
};
