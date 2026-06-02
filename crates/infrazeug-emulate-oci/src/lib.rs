//! OCI emulation via podman/docker (SOUL §5.1).
//!
//! Implements [`EmulatedHost`] for [`infrazeug_emulate`]: build images from a
//! [`BuildGraph`] (LLB lowered to Containerfile), run detached containers, and
//! exec into them for probe/apply. [`PodmanHost`] is the primary backend;
//! [`resolve_container_cli`] picks podman vs docker from `PATH`.
//!
//! [`EmulatedHost`]: infrazeug_emulate::EmulatedHost
//! [`BuildGraph`]: infrazeug_emulate::BuildGraph

pub mod container;
pub mod containerfile;
pub mod driver;
pub mod exec;
pub mod podman;

#[cfg(test)]
mod stack;

pub use container::{resolve_container_cli, ContainerCli, OciRuntimeKind, PodmanCli};
pub use driver::{build_graph, container_name, PodmanHost};
pub use exec::{ContainerExec, PodmanExec};
