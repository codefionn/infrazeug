//! MikroTik RouterOS tier-1 native nodes for infrazeug playbooks.
//!
//! Thin bridge over [`infrazeug_ext_mikrotik_api`] built on the shared
//! [`infrazeug_resource`] resource interface.
//!
//! # Example
//!
//! ```ignore
//! use infrazeug_api::builder::{self, InfraBuilder};
//! use infrazeug_mikrotik::{client_from_env, MikrotikInfraExt, EnsureIpAddressInput};
//! use infrazeug_core::id::{MachineId, NodeId};
//! use uuid::Uuid;
//!
//! let local = MachineId(Uuid::new_v4());
//! let addr = NodeId(Uuid::new_v4());
//! let bundle = InfraBuilder::new()
//!     .machine(builder::controller(local))?
//!     .mikrotik(client_from_env()?, local)
//!     .ensure_ip_address(addr, "mgmt", EnsureIpAddressInput {
//!         address: "192.168.88.2/24".into(),
//!         interface: "bridge".into(),
//!         ..Default::default()
//!     })?
//!     .finish();
//! ```

mod builder;
mod client;
mod methods;
mod registry;

pub use builder::{MikrotikInfraBuilder, MikrotikInfraExt};
pub use client::{client_from_env, MikrotikClientSource, MikrotikParams};
pub use methods::{
    ensure_firewall_rule, ensure_ip_address, EnsureFirewallRule, EnsureFirewallRuleInput,
    EnsureFirewallRuleOutput, EnsureIpAddress, EnsureIpAddressInput, EnsureIpAddressOutput,
    ENSURE_FIREWALL_RULE, ENSURE_IP_ADDRESS,
};
pub use registry::method_registry;

pub use infrazeug_ext_mikrotik_api::{Credentials, MikrotikClient, MikrotikConfig};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
