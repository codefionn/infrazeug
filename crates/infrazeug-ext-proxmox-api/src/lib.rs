//! Proxmox VE API client for infrazeug.
//!
//! A typed, async client for the [Proxmox VE API](https://pve.proxmox.com/pve-docs/api-viewer/).
//! It speaks the `/api2/json` surface, unwraps the `{ "data": ... }` envelope, and
//! sends mutating requests as `application/x-www-form-urlencoded` (as Proxmox
//! expects). Two credential styles are supported:
//!
//! - [`Auth::api_token`] — `PVEAPIToken=user@realm!tokenid=secret` (recommended).
//! - [`Auth::ticket`] — username/password login exchanged for a ticket + CSRF token.
//!
//! Typed bindings cover the two guest types:
//!
//! - [`qemu`] — QEMU/KVM virtual machines
//! - [`lxc`] — LXC containers
//!
//! Proxmox hosts usually present self-signed certificates; use
//! [`ProxmoxConfig::insecure_tls`] to accept them.
//!
//! # Example
//!
//! ```no_run
//! use infrazeug_ext_proxmox_api::{Auth, ProxmoxClient, ProxmoxConfig};
//! use infrazeug_ext_proxmox_api::qemu::QemuCreate;
//!
//! # async fn run() -> infrazeug_ext_proxmox_api::Result<()> {
//! let client = ProxmoxClient::new(
//!     ProxmoxConfig::new(
//!         "https://pve.example.com:8006",
//!         Auth::api_token("root@pam!automation", std::env::var("PVE_TOKEN").unwrap()),
//!     )
//!     .insecure_tls(true),
//! );
//!
//! let vms = client.qemu_list("pve").await?;
//! println!("vms on pve: {}", vms.len());
//!
//! let _upid = client
//!     .qemu_create(
//!         "pve",
//!         &QemuCreate {
//!             vmid: 100,
//!             name: Some("web-1".into()),
//!             cores: Some(2),
//!             memory: Some(2048),
//!             net0: Some("virtio,bridge=vmbr0".into()),
//!             ..Default::default()
//!         },
//!     )
//!     .await?;
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod lxc;
pub mod nodes;
pub mod qemu;
pub mod tasks;

mod auth;
mod client;

pub use auth::{Auth, TicketProvider};
pub use client::{ProxmoxClient, ProxmoxConfig};
pub use error::{ProxmoxError, Result};
pub use tasks::{TaskStatus, WaitOptions};
