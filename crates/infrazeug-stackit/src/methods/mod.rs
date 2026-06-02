//! Tier-1 resource methods for STACKIT IaaS.

mod server;
mod volume;

pub use server::{
    ensure_server, EnsureServer, EnsureServerInput, EnsureServerOutput, ENSURE_SERVER,
};
pub use volume::{
    ensure_volume, EnsureVolume, EnsureVolumeInput, EnsureVolumeOutput, ENSURE_VOLUME,
};
