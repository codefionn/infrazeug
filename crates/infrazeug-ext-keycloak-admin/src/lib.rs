//! Keycloak Admin REST API client for infrazeug.
//!
//! A typed, async client for the [Keycloak Admin REST API](https://www.keycloak.org/docs-api/latest/rest-api/).
//! It handles Bearer-token acquisition via `client_credentials` or `password`
//! grants and exposes typed bindings for the main resource surfaces:
//!
//! - [`realms`] — realm CRUD and management
//! - [`users`] — user lifecycle, credentials, role mappings
//! - [`clients`] — client configuration, secrets, scopes
//! - [`roles`] — realm and client roles, composites
//! - [`groups`] — groups and sub-groups
//! - [`client_scopes`] — client scope and protocol mapper management
//! - [`identity_providers`] — identity provider and mapper CRUD
//! - [`components`] — realm components (key providers, user federation, …)
//!
//! # Authentication
//!
//! The client authenticates via OAuth2 against
//! `/realms/{realm}/protocol/openid-connect/token`. Two grant types are
//! supported out of the box:
//!
//! - **Service account** (`client_credentials`) — for machine-to-machine usage.
//! - **Password** (`password`) — for admin-cli style access.
//!
//! The access token is transparently refreshed when it expires.
//!
//! # Example
//!
//! ```no_run
//! use infrazeug_ext_keycloak_admin::{KeycloakClient, KeycloakConfig, GrantType};
//!
//! # async fn run() -> infrazeug_ext_keycloak_admin::Result<()> {
//! let config = KeycloakConfig::new(
//!     "https://keycloak.example.local",
//!     "master",
//!     GrantType::ClientCredentials {
//!         client_id: "admin-cli".into(),
//!         client_secret: "secret".into(),
//!     },
//! );
//! let client = KeycloakClient::new(config);
//!
//! let realms = client.realms().await?;
//! for r in &realms {
//!     println!("realm: {} ({:?})", r.realm.as_deref().unwrap_or("-"), r.id);
//! }
//! # Ok(())
//! # }
//! ```

pub mod client_scopes;
pub mod clients;
pub mod components;
pub mod error;
pub mod groups;
pub mod identity_providers;
pub mod realms;
pub mod roles;
pub mod types;
pub mod users;

mod client;

pub use client::{GrantType, KeycloakClient, KeycloakConfig};
pub use error::{KeycloakError, Result};
