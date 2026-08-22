//! NetBird native nodes for infrazeug.
//!
//! The crate manages account configuration through the NetBird Management API.
//! Credentials can be supplied directly or read from either a regular controller
//! vault file or a generated mutable vault file only when a node is applied.
//! A mutable vault source reads `token` or `oauth_token`, plus an optional `host`,
//! from `files/mutable/{file}`. This is useful for a locally hosted NetBird
//! control plane whose API credential is generated during bootstrap.
//!
//! Removing an ensure node does not delete the remote object. Existing objects
//! are adopted by their documented natural key, usually their exact name.
//!
//! ```no_run
//! use infrazeug_api::builder::{self, InfraBuilder};
//! use infrazeug_core::id::{MachineId, NodeId};
//! use infrazeug_netbird::{
//!     EnsureNetworkInput, EnsureNetworkResourceInput, NetBirdInfraExt,
//! };
//! use uuid::Uuid;
//!
//! let controller = MachineId(Uuid::new_v4());
//! let network_node = NodeId(Uuid::new_v4());
//! let resource_node = NodeId(Uuid::new_v4());
//!
//! let bundle = InfraBuilder::new()
//!     .machine(builder::controller(controller))?
//!     .netbird_vault("network/netbird.vault", controller)
//!     .ensure_network(
//!         network_node,
//!         "office",
//!         EnsureNetworkInput {
//!             name: "office".into(),
//!             description: Some("Office services".into()),
//!         },
//!     )?
//!     .ensure_network_resource_after(
//!         resource_node,
//!         "git",
//!         EnsureNetworkResourceInput {
//!             network_name: Some("office".into()),
//!             name: "git".into(),
//!             address: "git.internal.example".into(),
//!             ..Default::default()
//!         },
//!         [network_node],
//!     )?
//!     .finish();
//! # Ok::<(), anyhow::Error>(())
//! ```

mod builder;
mod client;
mod methods;
mod registry;

pub use builder::{NetBirdInfraBuilder, NetBirdInfraExt};
pub use client::{client_from_env, NetBirdClientSource};
pub use methods::*;
pub use registry::method_registry;

pub use infrazeug_ext_netbird_api::{Auth, NetBirdClient, NetBirdConfig};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
