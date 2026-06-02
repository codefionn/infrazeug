//! Emulation types, BuildGraph, LLB lowering, and lock file (SOUL §5).
//!
//! Run the same playbook against emulated targets: OCI containers
//! ([`ContainerSpec`], [`BuildGraph`]) or QEMU microVMs. [`LikeConfig`] maps
//! a remote machine to a local twin for realistic tests; [`LockFile`]
//! (`infrazeug.lock`) pins image digests for reproducible builds.
//!
//! Drivers are split for optional dependencies:
//!
//! - `infrazeug-emulate-oci` — podman/docker via [`EmulatedHost`].
//! - `infrazeug-emulate-qemu` — cloud-init + SSH guests on an internal L2.
//!
//! Test mode registers a [`RunGuard`] (in `infrazeug-core`) so containers and
//! VMs are torn down when the run finishes.
//!
//! [`ContainerSpec`]: spec::ContainerSpec
//! [`BuildGraph`]: graph::BuildGraph
//! [`LikeConfig`]: like::LikeConfig
//! [`LockFile`]: lock::LockFile
//! [`EmulatedHost`]: host::EmulatedHost
//! [`RunGuard`]: infrazeug_core::RunGuard

pub mod digest;
pub mod error;
pub mod graph;
pub mod host;
pub mod like;
pub mod llb;
pub mod lock;
pub mod spec;

pub use digest::ContentDigest;
pub use error::{EmulateError, Result};
pub use graph::BuildGraph;
pub use host::{specs_for_container, BuiltImage, EmulatedHost, RunningContainer};
pub use like::{validate_like, LikeVars};
pub use llb::{graph_digest, lower_spec, Definition};
pub use lock::{LockContext, LockFile};
pub use spec::*;
