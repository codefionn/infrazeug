//! Built-in tier-1 method name constants (SOUL §3.3).
//!
//! Tier-1 methods run arbitrary Rust on the agent only; tier-2 [`ShellOp`]
//! nodes cover most file and command work. This crate holds stable `method`
//! string constants ([`SHELL`], [`FILE_READ`], …) shared by the agent
//! interpreter and plan-time validation so native-only methods cannot be
//! scheduled on agentless transports.
//!
//! [`ShellOp`]: infrazeug_shell::ShellOp

pub const SHELL: &str = "shell";
pub const FILE_READ: &str = "file.read";
pub const FILE_WRITE: &str = "file.write";
pub const FILE_DELETE: &str = "file.delete";
pub const FILE_LIST: &str = "file.list";
