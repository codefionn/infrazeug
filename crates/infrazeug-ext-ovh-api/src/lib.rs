//! OVHcloud API client for infrazeug.
//!
//! A dependency-light client that handles OVH's signed-request scheme and
//! exposes typed bindings for every OVHcloud API v2 product branch plus the
//! v1 surfaces infrazeug uses today:
//!
//! - [`alldom`] — **allDom** on API v1 (`/1.0/allDom`)
//! - [`domain`] — **domain** on API v2 (`/v2/domain`, names, AllDom, tasks)
//! - [`credential`] — consumer-key bootstrap (`POST /auth/credential`)
//! - [`me`] — account identity (`GET /me`)
//! - [`public_cloud`] — Public Cloud compute, block volumes, object storage (v1 `/cloud/project`)
//! - [`backup`] — Backup Services (v2 `/backupServices`)
//! - [`v2`] — all other API v2 branches: IAM, location, OKMS, managed CMS,
//!   network defense, notification, publicCloud (v2), VMware Cloud Director,
//!   vRack Services, web hosting, Zimbra, commercial catalog, videocenter
//!
//! # Authentication
//!
//! Two methods are supported:
//!
//! ## Classic (Application Key / Consumer Key)
//!
//! Create an application at <https://eu.api.ovh.com/createApp/> to obtain an
//! application key/secret, then validate a consumer key at
//! <https://eu.api.ovh.com/createToken/>. Every authenticated request is signed
//! with `SHA1(secret + consumerKey + method + url + body + timestamp)`; the
//! client transparently synchronises its clock with the API via `/auth/time`.
//!
//! ## OAuth2 (Service Account / IAM)
//!
//! Create an OAuth2 service account via `POST /me/api/oauth2/client` to obtain
//! a `client_id` / `client_secret` pair. The client fetches a short-lived
//! Bearer token via the OAuth2 client-credentials flow and sends
//! `Authorization: Bearer <token>` on every request. Tokens are cached and
//! automatically refreshed before expiry.
//!
//! # Example (Classic)
//!
//! ```no_run
//! use infrazeug_ext_ovh_api::{OvhClient, OvhEndpoint};
//!
//! # async fn run() -> infrazeug_ext_ovh_api::Result<()> {
//! let client = OvhClient::new(
//!     OvhEndpoint::OvhEu,
//!     std::env::var("OVH_APPLICATION_KEY").unwrap(),
//!     std::env::var("OVH_APPLICATION_SECRET").unwrap(),
//!     std::env::var("OVH_CONSUMER_KEY").unwrap(),
//! );
//!
//! for name in client.alldom_services().await? {
//!     let infos = client.alldom_service_infos(&name).await?;
//!     println!("{name}: expires {}", infos.expiration);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Example (OAuth2)
//!
//! ```no_run
//! use infrazeug_ext_ovh_api::{OvhClient, OvhEndpoint};
//!
//! # async fn run() -> infrazeug_ext_ovh_api::Result<()> {
//! let client = OvhClient::oauth2(
//!     OvhEndpoint::OvhEu,
//!     std::env::var("OVH_CLIENT_ID").unwrap(),
//!     std::env::var("OVH_CLIENT_SECRET").unwrap(),
//! );
//!
//! for name in client.alldom_services().await? {
//!     let infos = client.alldom_service_infos(&name).await?;
//!     println!("{name}: expires {}", infos.expiration);
//! }
//! # Ok(())
//! # }
//! ```

pub mod alldom;
mod auth;
pub mod backup;
mod client;
pub mod credential;
pub mod domain;
mod error;
pub mod iam;
pub mod me;
pub mod public_cloud;
pub mod v2;

pub use auth::{AuthMethod, Credentials, OAuth2Credentials};
pub use client::{OvhClient, OvhEndpoint, Page, PageParams, V2PageInfo, V2RequestOptions};
pub use error::{OvhError, Result};
pub use iam::ResourceMetadata;
