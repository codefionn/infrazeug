//! Tier-1 resource methods for Proxmox VE.

mod lxc;
mod qemu;
mod wait;

pub use lxc::{ensure_lxc, EnsureLxc, EnsureLxcInput, EnsureLxcOutput, ENSURE_LXC};
pub use qemu::{ensure_qemu, EnsureQemu, EnsureQemuInput, EnsureQemuOutput, ENSURE_QEMU};
