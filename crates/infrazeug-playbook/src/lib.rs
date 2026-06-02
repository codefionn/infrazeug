//! Discover playbook crates in the current directory and run them via a native build.
//!
//! Used by the `infrazeug-cli` binary when the user runs stock subcommands from a repo
//! that contains a playbook `Cargo.toml`. Flow for `plan|apply|test|lint|graph|mcp`:
//!
//! 1. `cargo build` the playbook binary (native host triple).
//! 2. `playbook __infrazeug-probe` → SSH targets + transport choices ([`PROBE_SUBCOMMAND`]).
//! 3. Probe `uname -m` over SSH where needed; cross-build `infrazeug-agent` per triple.
//! 4. `exec` the native playbook binary with the original argv (or watch + restart for `mcp serve`).
//!
//! [`discover_playbook`] walks upward from cwd; [`run_playbook_command`] performs
//! the build/probe/exec sequence.
//!

mod arch_probe;
mod discover;
mod mcp_watch;
mod run;

pub use discover::{discover_playbook, PlaybookProject};
pub use mcp_watch::run_mcp_watch;
pub use run::{
    is_forwarded_subcommand, is_playbook_subcommand, prepare_playbook, run_playbook_command,
};

pub const PROBE_SUBCOMMAND: &str = "__infrazeug-probe";
