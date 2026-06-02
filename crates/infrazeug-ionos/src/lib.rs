//! IONOS Cloud tier-1 native nodes for infrazeug playbooks.
//!
//! Thin bridge over [`infrazeug_ext_ionos_cloud_api`] built on the shared
//! [`infrazeug_resource`] resource interface: registers
//! [`Resource`](infrazeug_resource::Resource) implementations as tier-1
//! node methods and exposes [`IonosInfraBuilder`] helpers so playbooks do not
//! hand-roll method registration or idempotent ensure logic.
//!
//! # Example
//!
//! ```ignore
//! use infrazeug_api::builder::{self, InfraBuilder};
//! use infrazeug_ionos::{client_from_env, IonosInfraExt, EnsureServerInput};
//! use infrazeug_core::id::{MachineId, NodeId};
//! use uuid::Uuid;
//!
//! let local = MachineId(Uuid::new_v4());
//! let server = NodeId(Uuid::new_v4());
//! let bundle = InfraBuilder::new()
//!     .machine(builder::controller(local))?
//!     .ionos(client_from_env()?, local)
//!     .ensure_server(server, "web-1", EnsureServerInput {
//!         datacenter_id: "dc-1".into(),
//!         name: "web-1".into(),
//!         cores: 2,
//!         ram: 4096,
//!         availability_zone: Some("AUTO".into()),
//!         cpu_family: None,
//!     })?
//!     .finish();
//! ```

mod builder;
mod client;
mod methods;
mod registry;

pub use builder::{IonosInfraBuilder, IonosInfraExt};
pub use client::{client_from_env, IonosClientSource};
pub use methods::{
    ensure_server, ensure_volume, EnsureServer, EnsureServerInput, EnsureServerOutput,
    EnsureVolume, EnsureVolumeInput, EnsureVolumeOutput, ENSURE_SERVER, ENSURE_VOLUME,
};
pub use registry::method_registry;

pub use infrazeug_ext_ionos_cloud_api::{Auth, IonosClient, IonosConfig};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
