//! Keycloak tier-1 native nodes for infrazeug playbooks.
//!
//! Thin bridge over [`infrazeug_ext_keycloak_admin`]: each managed Keycloak surface
//! (realm, client, user) implements [`Resource`](infrazeug_resource::Resource)
//! and is exposed as a [`NodeMethod`](infrazeug_native::NodeMethod) via
//! [`EnsureResource`](infrazeug_resource::EnsureResource), so it joins the infrazeug node
//! graph and reuses the existing idempotency, plan/diff, capture→vault, and retry
//! machinery — exactly like the cloud providers (`infrazeug-ovh`, `infrazeug-ionos`).
//!
//! Unlike an immutable cloud bucket, Keycloak objects are declarative and mutable, so
//! each resource implements a *narrow* `diff` (only the spec-declared fields) plus a
//! `reconcile` that GETs the full representation, applies the managed fields, and PUTs
//! it back — keeping server-filled defaults intact.
//!
//! # Example
//!
//! ```ignore
//! use infrazeug_api::builder::{self, InfraBuilder};
//! use infrazeug_keycloak::{client_from_env, EnsureRealmInput, KeycloakInfraExt};
//! use infrazeug_core::id::{MachineId, NodeId};
//! use uuid::Uuid;
//!
//! let local = MachineId(Uuid::new_v4());
//! let realm = NodeId(Uuid::new_v4());
//! let bundle = InfraBuilder::new()
//!     .machine(builder::controller(local))?
//!     .keycloak(client_from_env()?, local)
//!     .ensure_realm(realm, "tenant-realm", EnsureRealmInput {
//!         realm: "tenant".into(),
//!         enabled: Some(true),
//!         display_name: Some("Tenant".into()),
//!         ..Default::default()
//!     })?
//!     .finish();
//! ```

mod builder;
mod client;
mod methods;
mod registry;

pub use builder::{KeycloakInfraBuilder, KeycloakInfraExt};
pub use client::{client_from_env, KeycloakClientSource};
pub use methods::{
    ensure_client, ensure_client_role, ensure_realm, ensure_user, EnsureClient, EnsureClientInput,
    EnsureClientOutput, EnsureClientRole, EnsureClientRoleInput, EnsureClientRoleOutput,
    EnsureRealm, EnsureRealmInput, EnsureRealmOutput, EnsureUser, EnsureUserInput,
    EnsureUserOutput, ENSURE_CLIENT, ENSURE_CLIENT_ROLE, ENSURE_REALM, ENSURE_USER,
};
pub use registry::method_registry;

pub use infrazeug_ext_keycloak_admin::{GrantType, KeycloakClient, KeycloakConfig};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
