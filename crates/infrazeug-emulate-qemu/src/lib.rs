//! QEMU/KVM microVM driver: cloud-init seed + SSH guest (SOUL §5.2).
//!
//! Boots Debian cloud images on a user-mode networking stack (test-focused),
//! injects SSH keys and stack config via [`CloudInitSeed`], and exposes
//! [`QemuHost`] as an [`EmulatedHost`]. Requires `qemu-system-*` and
//! optionally `qemu-img` for copy-on-write overlays.
//!
//! [`EmulatedHost`]: infrazeug_emulate::EmulatedHost

mod cloud_init;
#[cfg(test)]
mod guest_ssh;
mod host;
mod overlay;
mod spawn;

#[cfg(test)]
mod k3s_stack;
#[cfg(test)]
mod vm_stack;

pub use cloud_init::{CloudInitSeed, SshGuestConfig, StackGuestConfig};
pub use host::{boot_microvm, stop_microvm, MicroVmBoot, MicroVmHandle, QemuHost};
pub use overlay::{create_overlay, qemu_img_available};
pub use spawn::{qemu_available, QemuArch};
