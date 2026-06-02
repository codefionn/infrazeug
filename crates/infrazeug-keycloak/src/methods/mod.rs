//! Tier-1 resource methods for Keycloak.
//!
//! Each surface implements [`infrazeug_resource::Resource`] and is exposed as a
//! [`NodeMethod`](infrazeug_native::NodeMethod) via
//! [`EnsureResource`](infrazeug_resource::EnsureResource).

mod client;
mod client_role;
mod realm;
mod user;

pub use client::{
    ensure_client, EnsureClient, EnsureClientInput, EnsureClientOutput, ENSURE_CLIENT,
};
pub use client_role::{
    ensure_client_role, EnsureClientRole, EnsureClientRoleInput, EnsureClientRoleOutput,
    ENSURE_CLIENT_ROLE,
};
pub use realm::{ensure_realm, EnsureRealm, EnsureRealmInput, EnsureRealmOutput, ENSURE_REALM};
pub use user::{ensure_user, EnsureUser, EnsureUserInput, EnsureUserOutput, ENSURE_USER};
