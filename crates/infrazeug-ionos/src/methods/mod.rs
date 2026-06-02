//! Tier-1 resource methods for IONOS Cloud.
//!
//! Each resource implements [`infrazeug_resource::Resource`] and is exposed as a
//! [`NodeMethod`](infrazeug_native::NodeMethod) via
//! [`EnsureResource`](infrazeug_resource::EnsureResource).

mod datacenter;
mod server;
mod volume;

pub use datacenter::{ensure_datacenter, EnsureDatacenter, EnsureDatacenterInput};
pub use server::{
    ensure_server, EnsureServer, EnsureServerInput, EnsureServerOutput, ENSURE_SERVER,
};
pub use volume::{
    ensure_volume, EnsureVolume, EnsureVolumeInput, EnsureVolumeOutput, ENSURE_VOLUME,
};
