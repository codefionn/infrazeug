//! Tier-1 resource methods for MikroTik RouterOS.

mod firewall_rule;
mod ip_address;

pub use firewall_rule::{
    ensure_firewall_rule, EnsureFirewallRule, EnsureFirewallRuleInput, EnsureFirewallRuleOutput,
    ENSURE_FIREWALL_RULE,
};
pub use ip_address::{
    ensure_ip_address, EnsureIpAddress, EnsureIpAddressInput, EnsureIpAddressOutput,
    ENSURE_IP_ADDRESS,
};
