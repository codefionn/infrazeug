//! STACKIT IaaS API client for infrazeug.
//!
//! A typed, async client for the [STACKIT IaaS API](https://docs.api.stackit.cloud/)
//! (v1 regional hosts and v2 multi-region paths). It supports service-account
//! token authentication and the recommended key flow (RSA-signed JWT exchange),
//! and exposes typed bindings for the main compute surfaces:
//!
//! - [`servers`] — server lifecycle inside a project
//! - [`volumes`] — block storage volumes
//!
//! # Authentication
//!
//! Create a service account in the STACKIT Portal and either:
//!
//! - pass a long-lived access token via [`Auth::token`], or
//! - use the key flow with [`Auth::service_account_key`] / [`Auth::service_account_key_json`].
//!
//! # Example
//!
//! ```no_run
//! use infrazeug_ext_stackit_api::{Auth, StackitClient, StackitConfig};
//! use infrazeug_ext_stackit_api::volumes::VolumeCreate;
//! use infrazeug_ext_stackit_api::types::ResourceSource;
//!
//! # async fn run() -> infrazeug_ext_stackit_api::Result<()> {
//! let client = StackitClient::new(StackitConfig::new(Auth::token(
//!     std::env::var("STACKIT_SERVICE_ACCOUNT_TOKEN").unwrap(),
//! )));
//!
//! let volumes = client.volumes("my-project-id").await?;
//! println!("volumes: {}", volumes.items.len());
//!
//! let _created = client.create_volume(
//!     "my-project-id",
//!     &VolumeCreate {
//!         name: "data".into(),
//!         size: 10,
//!         availability_zone: Some("eu01-1".into()),
//!         source: Some(ResourceSource {
//!             id: "image-id".into(),
//!             source_type: "image".into(),
//!         }),
//!         performance_class: None,
//!     },
//! ).await?;
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod servers;
pub mod types;
pub mod volumes;

mod auth;
mod client;

pub use auth::{Auth, KeyFlowProvider, ServiceAccountKey, ServiceAccountKeyCredentials};
pub use client::{StackitClient, StackitConfig};
pub use error::{Result, StackitError};
