//! NetBird Management API client for infrazeug.
//!
//! This crate binds the public Management API used by NetBird Cloud and
//! self-hosted management servers. It supports personal access tokens and OAuth
//! bearer tokens. The default endpoint is `https://api.netbird.io`; use
//! [`NetBirdConfig::with_host`] for self-hosting.
//!
//! ```no_run
//! use infrazeug_ext_netbird_api::{Auth, NetBirdClient, NetBirdConfig};
//!
//! # async fn run() -> infrazeug_ext_netbird_api::Result<()> {
//! let client = NetBirdClient::new(NetBirdConfig::new(Auth::personal_access_token(
//!     std::env::var("NETBIRD_TOKEN").unwrap(),
//! )));
//! for peer in client.peers(None, None).await? {
//!     println!("{}", peer.name.unwrap_or_default());
//! }
//! # Ok(())
//! # }
//! ```

mod auth;
mod client;
mod error;
pub mod types;

pub use auth::Auth;
pub use client::{NetBirdClient, NetBirdConfig, DEFAULT_HOST};
pub use error::{NetBirdError, Result};
pub use types::*;
