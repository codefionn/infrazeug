//! Proxmox VE tier-1 native nodes for infrazeug playbooks.
//!
//! Thin bridge over [`infrazeug_ext_proxmox_api`] built on the shared
//! [`infrazeug_resource`] resource interface. It exposes two ensure resources —
//! QEMU/KVM virtual machines and LXC containers — keyed by `node` + `vmid`, with
//! create-if-absent semantics and drift reconciliation on CPU/memory/name.
//!
//! Credentials come from the environment ([`client_from_env`]) or the controller
//! vault ([`ProxmoxInfraExt::proxmox_vault`]); they are never required as apply-time
//! environment variables.
//!
//! # Example
//!
//! ```ignore
//! use infrazeug_api::builder::{self, InfraBuilder};
//! use infrazeug_proxmox::{client_from_env, ProxmoxInfraExt, EnsureQemuInput, EnsureLxcInput};
//! use infrazeug_core::id::{MachineId, NodeId};
//! use uuid::Uuid;
//!
//! let local = MachineId(Uuid::new_v4());
//! let vm = NodeId(Uuid::new_v4());
//! let bundle = InfraBuilder::new()
//!     .machine(builder::controller(local))?
//!     .proxmox(client_from_env()?, local)
//!     .ensure_qemu(vm, "web-1", EnsureQemuInput {
//!         node: "pve".into(),
//!         vmid: 100,
//!         name: Some("web-1".into()),
//!         cores: Some(2),
//!         memory: Some(2048),
//!         net0: Some("virtio,bridge=vmbr0".into()),
//!         ..Default::default()
//!     })?
//!     .finish();
//! ```

mod builder;
mod client;
mod methods;
mod registry;

pub use builder::{ProxmoxInfraBuilder, ProxmoxInfraExt};
pub use client::{client_from_env, ProxmoxClientSource};
pub use methods::{
    ensure_lxc, ensure_qemu, EnsureLxc, EnsureLxcInput, EnsureLxcOutput, EnsureQemu,
    EnsureQemuInput, EnsureQemuOutput, ENSURE_LXC, ENSURE_QEMU,
};
pub use registry::method_registry;

pub use infrazeug_ext_proxmox_api::{Auth, ProxmoxClient, ProxmoxConfig};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
