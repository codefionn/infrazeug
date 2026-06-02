//! STACKIT IaaS tier-1 native nodes for infrazeug playbooks.
//!
//! Thin bridge over [`infrazeug_ext_stackit_api`] built on the shared
//! [`infrazeug_resource`] resource interface.
//!
//! # Example
//!
//! ```ignore
//! use infrazeug_api::builder::{self, InfraBuilder};
//! use infrazeug_stackit::{client_from_env, StackitInfraExt, EnsureVolumeInput, EnsureServerInput};
//! use infrazeug_core::id::{MachineId, NodeId};
//! use uuid::Uuid;
//!
//! let local = MachineId(Uuid::new_v4());
//! let volume = NodeId(Uuid::new_v4());
//! let server = NodeId(Uuid::new_v4());
//! let bundle = InfraBuilder::new()
//!     .machine(builder::controller(local))?
//!     .stackit(client_from_env()?, local)
//!     .ensure_volume(volume, "boot", EnsureVolumeInput {
//!         project_id: "proj-1".into(),
//!         name: "boot".into(),
//!         size: 10,
//!         availability_zone: Some("eu01-1".into()),
//!         source_id: Some("image-id".into()),
//!         source_type: "image".into(),
//!         performance_class: None,
//!         region: None,
//!     })?
//!     .ensure_server(server, "web-1", EnsureServerInput {
//!         project_id: "proj-1".into(),
//!         name: "web-1".into(),
//!         machine_type: "g2i.1".into(),
//!         boot_volume_id: "volume-id".into(),
//!         boot_volume_source_type: "volume".into(),
//!         availability_zone: None,
//!         keypair_name: None,
//!         network_id: None,
//!         security_groups: None,
//!         region: None,
//!     })?
//!     .finish();
//! ```

mod builder;
mod client;
mod methods;
mod registry;

pub use builder::{StackitInfraBuilder, StackitInfraExt};
pub use client::{client_from_env, StackitClientSource};
pub use methods::{
    ensure_server, ensure_volume, EnsureServer, EnsureServerInput, EnsureServerOutput,
    EnsureVolume, EnsureVolumeInput, EnsureVolumeOutput, ENSURE_SERVER, ENSURE_VOLUME,
};
pub use registry::method_registry;

pub use infrazeug_ext_stackit_api::{Auth, StackitClient, StackitConfig};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
