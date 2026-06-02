//! UniFi Network controller API client for infrazeug.
//!
//! A typed, async client for the controller's site-scoped REST API (the surface
//! behind the Network web UI: `wlanconf`, `networkconf`, `portforward`, …). It
//! supports both UniFi OS consoles (UDM / Cloud Key Gen2+, API under
//! `/proxy/network`) and legacy standalone controllers, with username/password
//! session login (cookie + rotating CSRF token) or an `X-API-KEY`.
//!
//! Typed bindings cover:
//!
//! - [`wlan`] — wireless networks (SSIDs)
//! - [`network`] — LANs / VLAN-only networks
//! - [`port_forward`] — destination-NAT rules
//! - [`dns`] — local DNS records (v2 API)
//! - [`firewall_group`] / [`firewall_rule`] — address/port sets and firewall rules
//! - [`user_group`] — per-client bandwidth profiles
//! - [`users`] — known clients (naming, fixed-IP reservations)
//! - [`device`] — APs/switches/gateways: state, settings, restart/adopt/provision
//! - [`stations`] — active clients: live list and block/unblock/reconnect/forget
//! - [`health`] / [`sysinfo`] — site health and controller system info
//! - [`sites`] — site listing
//!
//! # Example
//!
//! ```no_run
//! use infrazeug_ext_unifi_api::{Credentials, UnifiClient, UnifiConfig};
//! use infrazeug_ext_unifi_api::network::NetworkConf;
//!
//! # async fn run() -> infrazeug_ext_unifi_api::Result<()> {
//! let client = UnifiClient::new(
//!     UnifiConfig::new(
//!         "https://192.168.1.1",
//!         Credentials::user_pass("admin", std::env::var("UNIFI_PASSWORD").unwrap()),
//!     )
//!     .insecure(), // UniFi ships a self-signed cert; skip TLS verification
//! );
//!
//! let _created = client
//!     .create_network(&NetworkConf {
//!         name: "iot".into(),
//!         purpose: Some("vlan-only".into()),
//!         vlan_enabled: Some(true),
//!         vlan: Some(20),
//!         ..Default::default()
//!     })
//!     .await?;
//! # Ok(())
//! # }
//! ```

pub mod device;
pub mod dns;
pub mod error;
pub mod firewall_group;
pub mod firewall_rule;
pub mod health;
pub mod network;
pub mod port_forward;
pub mod sites;
pub mod stations;
pub mod sysinfo;
pub mod types;
pub mod user_group;
pub mod users;
pub mod wlan;

mod auth;
mod client;

pub use auth::Credentials;
pub use client::{ControllerKind, UnifiClient, UnifiConfig, DEFAULT_SITE};
pub use error::{Result, UnifiError};
pub use types::{Meta, UnifiResponse};
