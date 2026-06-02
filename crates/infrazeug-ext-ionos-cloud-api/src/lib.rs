//! IONOS Cloud API client for infrazeug.
//!
//! A typed, async client for the [IONOS Cloud API v6](https://api.ionos.com/docs/cloud/v6/)
//! and the companion [Auth API v1](https://api.ionos.com/docs/authentication/v1).
//! It handles Bearer-token or Basic authentication, optional
//! `X-Contract-Number` for multi-contract accounts, and exposes typed
//! bindings for the main resource surfaces:
//!
//! - [`datacenters`] — data center CRUD
//! - [`servers`] — server lifecycle inside a data center
//! - [`volumes`] — block storage volumes
//! - [`images`] / [`snapshots`] — image and snapshot management
//! - [`ipblocks`] — reserved public IP blocks
//! - [`lans`] / [`nics`] — virtual networking
//! - [`firewall_rules`] / [`security_groups`] — firewall policy
//! - [`load_balancers`] — classic load balancers
//! - [`kubernetes`] — managed Kubernetes clusters and node pools
//! - [`templates`] — Cube server templates (read-only)
//! - [`requests`] — async API request tracking and polling
//! - [`users`] / [`groups`] — user and group management (`/um/*`)
//! - [`um_resources`] — contract resource inventory
//! - [`s3_keys`] — per-user Object Storage keys
//! - [`locations`] — region/location discovery
//! - [`tokens`] — Auth API token management
//!
//! # Authentication
//!
//! Create a token in the IONOS Data Center Designer (Token Manager) or generate
//! one programmatically via [`IonosClient::generate_token`]. Pass it as
//! [`Auth::token`], or use [`Auth::basic`] when 2-Factor Authentication is not
//! enabled on the account.
//!
//! # Example
//!
//! ```no_run
//! use infrazeug_ext_ionos_cloud_api::{Auth, IonosClient, IonosConfig, ListQuery};
//!
//! # async fn run() -> infrazeug_ext_ionos_cloud_api::Result<()> {
//! let client = IonosClient::new(IonosConfig::new(Auth::token(
//!     std::env::var("IONOS_TOKEN").unwrap(),
//! )));
//!
//! let info = client.api_info().await?;
//! println!("API: {} {}", info.name.unwrap_or_default(), info.version.unwrap_or_default());
//!
//! let dcs = client.datacenters(&ListQuery::default()).await?;
//! for dc in dcs.items {
//!     if let Some(name) = dc.properties.as_ref().and_then(|p| p.name.as_deref()) {
//!         println!("datacenter: {name}");
//!     }
//! }
//! # Ok(())
//! # }
//! ```

pub mod datacenters;
pub mod error;
pub mod firewall_rules;
pub mod groups;
pub mod images;
pub mod ipblocks;
pub mod kubernetes;
pub mod lans;
pub mod load_balancers;
pub mod locations;
pub mod nics;
pub mod requests;
pub mod s3_keys;
pub mod security_groups;
pub mod servers;
pub mod snapshots;
pub mod templates;
pub mod tokens;
pub mod types;
pub mod um_resources;
pub mod users;
pub mod volumes;

mod auth;
mod client;
mod um_types;

pub use auth::Auth;
pub use client::{IonosClient, IonosConfig};
pub use error::{IonosError, Result};
pub use requests::{RequestStatusFilter, WaitRequestOptions};
pub use types::ListQuery;
