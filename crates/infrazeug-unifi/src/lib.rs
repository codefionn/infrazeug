//! UniFi Network controller tier-1 native nodes for infrazeug playbooks.
//!
//! Thin bridge over [`infrazeug_ext_unifi_api`] built on the shared
//! [`infrazeug_resource`] resource interface: each resource (`networkconf`,
//! `wlanconf`, `portforward`) implements [`Resource`] and is wrapped as a
//! tier-1 node method, so it participates in the infrazeug graph (deps, run-policy,
//! capture→vault, retry) like any other node.
//!
//! # Example
//!
//! ```ignore
//! use infrazeug_api::builder::{self, InfraBuilder};
//! use infrazeug_unifi::{client_from_env, UnifiInfraExt, EnsureNetworkInput, EnsureWlanInput};
//! use infrazeug_core::id::{MachineId, NodeId};
//! use uuid::Uuid;
//!
//! let local = MachineId(Uuid::new_v4());
//! let net = NodeId(Uuid::new_v4());
//! let wlan = NodeId(Uuid::new_v4());
//! let bundle = InfraBuilder::new()
//!     .machine(builder::controller(local))?
//!     .unifi(client_from_env()?, local)
//!     .ensure_network(net, "iot", EnsureNetworkInput {
//!         name: "iot".into(),
//!         purpose: "vlan-only".into(),
//!         vlan: Some(20),
//!         ..Default::default()
//!     })?
//!     .ensure_wlan(wlan, "iot-ssid", EnsureWlanInput {
//!         name: "iot".into(),
//!         security: "wpapsk".into(),
//!         passphrase: Some("supersecret".into()),
//!         ..Default::default()
//!     })?
//!     .finish();
//! ```

mod builder;
mod client;
mod methods;
mod registry;

pub use builder::{UnifiInfraBuilder, UnifiInfraExt};
pub use client::{client_from_env, UnifiClientSource};
pub use methods::{
    ensure_dns_record, ensure_firewall_group, ensure_firewall_rule, ensure_fixed_ip,
    ensure_network, ensure_port_forward, ensure_user_group, ensure_wlan, EnsureDnsRecord,
    EnsureDnsRecordInput, EnsureDnsRecordOutput, EnsureFirewallGroup, EnsureFirewallGroupInput,
    EnsureFirewallGroupOutput, EnsureFirewallRule, EnsureFirewallRuleInput,
    EnsureFirewallRuleOutput, EnsureFixedIp, EnsureFixedIpInput, EnsureFixedIpOutput,
    EnsureNetwork, EnsureNetworkInput, EnsureNetworkOutput, EnsurePortForward,
    EnsurePortForwardInput, EnsurePortForwardOutput, EnsureUserGroup, EnsureUserGroupInput,
    EnsureUserGroupOutput, EnsureWlan, EnsureWlanInput, EnsureWlanOutput, ENSURE_DNS_RECORD,
    ENSURE_FIREWALL_GROUP, ENSURE_FIREWALL_RULE, ENSURE_FIXED_IP, ENSURE_NETWORK,
    ENSURE_PORT_FORWARD, ENSURE_USER_GROUP, ENSURE_WLAN,
};
pub use registry::method_registry;

pub use infrazeug_ext_unifi_api::{ControllerKind, Credentials, UnifiClient, UnifiConfig};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
