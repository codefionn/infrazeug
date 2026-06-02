//! Backblaze B2 tier-1 native nodes for infrazeug playbooks.
//!
//! Thin bridge over [`infrazeug_ext_backblaze_api`] built on the shared
//! [`infrazeug_resource`] resource interface.
//!
//! # Example
//!
//! ```ignore
//! use infrazeug_api::builder::{self, InfraBuilder};
//! use infrazeug_backblaze::{client_from_env, BackblazeInfraExt, EnsureBucketInput};
//! use infrazeug_core::id::{MachineId, NodeId};
//! use uuid::Uuid;
//!
//! let local = MachineId(Uuid::new_v4());
//! let bucket = NodeId(Uuid::new_v4());
//! let bundle = InfraBuilder::new()
//!     .machine(builder::controller(local))?
//!     .backblaze(client_from_env()?, local)
//!     .ensure_bucket(bucket, "logs", EnsureBucketInput {
//!         bucket_name: "my-logs-bucket".into(),
//!         ..Default::default()
//!     })?
//!     .finish();
//! ```

mod builder;
mod client;
mod methods;
mod registry;

pub use builder::{BackblazeInfraBuilder, BackblazeInfraExt};
pub use client::{client_from_env, BackblazeClientSource};
pub use methods::{
    ensure_application_key, ensure_bucket, EnsureApplicationKey, EnsureApplicationKeyInput,
    EnsureApplicationKeyOutput, EnsureBucket, EnsureBucketInput, EnsureBucketOutput,
    ENSURE_APPLICATION_KEY, ENSURE_BUCKET,
};
pub use registry::method_registry;

pub use infrazeug_ext_backblaze_api::{BackblazeClient, BackblazeConfig, Credentials};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
