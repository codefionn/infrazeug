//! Tier-1 resource methods for the UniFi Network controller.

mod dns_record;
mod firewall_group;
mod firewall_rule;
mod fixed_ip;
mod network;
mod port_forward;
mod user_group;
mod wlan;

pub use dns_record::{
    ensure_dns_record, EnsureDnsRecord, EnsureDnsRecordInput, EnsureDnsRecordOutput,
    ENSURE_DNS_RECORD,
};
pub use firewall_group::{
    ensure_firewall_group, EnsureFirewallGroup, EnsureFirewallGroupInput,
    EnsureFirewallGroupOutput, ENSURE_FIREWALL_GROUP,
};
pub use firewall_rule::{
    ensure_firewall_rule, EnsureFirewallRule, EnsureFirewallRuleInput, EnsureFirewallRuleOutput,
    ENSURE_FIREWALL_RULE,
};
pub use fixed_ip::{
    ensure_fixed_ip, EnsureFixedIp, EnsureFixedIpInput, EnsureFixedIpOutput, ENSURE_FIXED_IP,
};
pub use network::{
    ensure_network, EnsureNetwork, EnsureNetworkInput, EnsureNetworkOutput, ENSURE_NETWORK,
};
pub use port_forward::{
    ensure_port_forward, EnsurePortForward, EnsurePortForwardInput, EnsurePortForwardOutput,
    ENSURE_PORT_FORWARD,
};
pub use user_group::{
    ensure_user_group, EnsureUserGroup, EnsureUserGroupInput, EnsureUserGroupOutput,
    ENSURE_USER_GROUP,
};
pub use wlan::{ensure_wlan, EnsureWlan, EnsureWlanInput, EnsureWlanOutput, ENSURE_WLAN};
