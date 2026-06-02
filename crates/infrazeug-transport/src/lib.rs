//! Transport abstraction (SOUL §4). M1: Local; M2: SSH agentless + agent push.
//!
//! # Transport microarchitecture
//!
//! The [`TransportFactory`] routes shell operations to the correct backend
//! per machine. In push mode, remote machines use either
//! [`AgentPushBackend`] (postcard RPC over stdio) or [`AgentlessBackend`]
//! (SSH/SFTP lowering). Local machines use [`LocalShellExecutor`] directly.
//! See `docs/protocol.md` for the full microarchitecture.

pub mod error;
pub mod factory;
pub mod local;
pub mod ssh;

pub use error::{Result, TransportError};
pub use factory::TransportFactory;
pub use infrazeug_core::transport::TransportChoice;
pub use local::LocalTransport;
pub use ssh::{probe_uname_machine, SshAuthResolver};
