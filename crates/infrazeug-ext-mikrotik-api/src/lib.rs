//! MikroTik RouterOS API client for infrazeug.
//!
//! Async client for the binary RouterOS API (TCP:8728 or API-SSL:8729). Commands
//! mirror the CLI (`/ip/address/print`, `/ip/firewall/filter/add`, …) over
//! length-prefixed words.
//!
//! Typed bindings cover:
//!
//! - [`ip_address`] — `/ip/address`
//! - [`interface`] — `/interface` (ether / vlan listing)
//! - [`firewall_filter`] — `/ip/firewall/filter`
//!
//! # Example
//!
//! ```no_run
//! use infrazeug_ext_mikrotik_api::{Credentials, MikrotikClient, MikrotikConfig};
//!
//! # async fn run() -> infrazeug_ext_mikrotik_api::Result<()> {
//! let mut client = MikrotikClient::new(
//!     MikrotikConfig::new("192.168.88.1"),
//!     Credentials::new("admin", std::env::var("MIKROTIK_PASSWORD").unwrap()),
//! );
//! client.connect().await?;
//! let addrs = client.ip_addresses().await?;
//! println!("{} addresses", addrs.len());
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod firewall_filter;
pub mod interface;
pub mod ip_address;

mod auth;
mod client;
mod wire;

pub use auth::Credentials;
pub use client::{MikrotikClient, MikrotikConfig, DEFAULT_PLAIN_PORT, DEFAULT_TLS_PORT};
pub use error::{MikrotikError, Result};
pub use wire::{decode_word, encode_word, Reply, Sentence};
