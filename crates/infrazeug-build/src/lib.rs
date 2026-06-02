//! Cross-compile and cache `infrazeug-agent` (SOUL §4.4b).
//!
//! Before push-mode apply, the controller builds one agent binary per remote
//! architecture and caches it under `target/infrazeug-agents/<triple>/`.
//! [`build_agent`] invokes `cargo build -p infrazeug-agent` with the requested
//! `--target`; [`AgentBuildOptions`] controls release vs debug and target list.
//!
//! Playbook discovery ([`infrazeug_playbook`]) probes `uname -m` over SSH when
//! needed, then calls into this crate so the correct agent is uploaded with
//! the plan.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const AGENT_CRATE: &str = "infrazeug-agent";
const AGENT_BIN: &str = "infrazeug-agent";

#[derive(Clone, Debug, Default)]
pub struct AgentBuildOptions {
    pub targets: Vec<String>,
    pub release: bool,
    /// Suppress cargo/zigbuild terminal output (for `--tui` / `--watch`).
    pub quiet: bool,
}

impl AgentBuildOptions {
    pub fn with_target(mut self, triple: impl Into<String>) -> Self {
        self.targets.push(triple.into());
        self
    }
}

/// Build the agent for the host triple (or requested targets) into `target/infrazeug-agents/<triple>/`.
pub fn build_agent(
    workspace_root: impl AsRef<Path>,
    opts: &AgentBuildOptions,
) -> Result<Vec<PathBuf>, String> {
    let workspace_root = workspace_root.as_ref();
    let targets: Vec<String> = if opts.targets.is_empty() {
        vec![host_triple().unwrap_or_else(|| "host".to_string())]
    } else {
        opts.targets.clone()
    };

    let profile = if opts.release { "release" } else { "debug" };
    let mut outputs = Vec::new();

    for triple in targets {
        let triple = normalize_target_triple(&triple);
        let out_dir = workspace_root.join("target/infrazeug-agents").join(&triple);
        std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
        let out_path = out_dir.join(AGENT_BIN);

        if triple == "host" {
            cargo_build_host(workspace_root, profile, opts.quiet)?;
            let built = workspace_root.join("target").join(profile).join(AGENT_BIN);
            if !built.is_file() {
                return Err(format!("agent binary not found at {}", built.display()));
            }
            std::fs::copy(&built, &out_path).map_err(|e| e.to_string())?;
            outputs.push(out_path);
            continue;
        }

        if try_zigbuild(workspace_root, &triple, profile, &out_path, opts.quiet)? {
            outputs.push(out_path);
            continue;
        }

        cargo_build(workspace_root, &triple, profile, opts.quiet)?;
        let built = workspace_root
            .join("target")
            .join(&triple)
            .join(profile)
            .join(AGENT_BIN);
        if !built.is_file() {
            let built_host = workspace_root.join("target").join(profile).join(AGENT_BIN);
            if built_host.is_file() {
                std::fs::copy(&built_host, &out_path).map_err(|e| e.to_string())?;
            } else {
                return Err(format!("agent binary not found at {}", built.display()));
            }
        } else {
            std::fs::copy(&built, &out_path).map_err(|e| e.to_string())?;
        }
        outputs.push(out_path);
    }
    Ok(outputs)
}

fn try_zigbuild(
    root: &Path,
    triple: &str,
    profile: &str,
    dest: &Path,
    quiet: bool,
) -> Result<bool, String> {
    if Command::new("zig").arg("version").output().is_err() {
        return Ok(false);
    }
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root)
        .args(["zigbuild", "-p", AGENT_CRATE, "--bin", AGENT_BIN])
        .arg("--target")
        .arg(triple);
    if profile == "release" {
        cmd.arg("--release");
    }
    configure_cargo_stdio(&mut cmd, quiet);
    let output = cmd.output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "cargo zigbuild for {triple} failed (install `cargo-zigbuild` and ensure `zig` works):\n{stderr}"
        ));
    }
    let built = root
        .join("target")
        .join(triple)
        .join(profile)
        .join(AGENT_BIN);
    if built.is_file() {
        std::fs::copy(&built, dest).map_err(|e| e.to_string())?;
        return Ok(true);
    }
    Err(format!(
        "cargo zigbuild succeeded but agent binary missing at {}",
        built.display()
    ))
}

fn cargo_build_host(root: &Path, profile: &str, quiet: bool) -> Result<(), String> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root)
        .args(["build", "-p", AGENT_CRATE, "--bin", AGENT_BIN]);
    if profile == "release" {
        cmd.arg("--release");
    }
    run_cargo(&mut cmd, quiet)
}

/// Workspace root containing the `infrazeug-agent` crate.
pub fn infrazeug_workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Accept either a full Rust triple or bare `uname -m` output.
pub fn normalize_target_triple(triple: &str) -> String {
    let t = triple.trim();
    if t == "host" || t.contains('-') {
        t.to_string()
    } else {
        uname_machine_to_triple(t)
    }
}

/// Map `uname -m` to a Rust target triple (musl for Linux portability).
pub fn uname_machine_to_triple(uname_m: &str) -> String {
    match uname_m.trim() {
        "x86_64" | "amd64" => "x86_64-unknown-linux-musl".into(),
        "aarch64" | "arm64" => "aarch64-unknown-linux-musl".into(),
        "armv7l" | "armv6l" => "armv7-unknown-linux-musleabihf".into(),
        "riscv64gc" | "riscv64" => "riscv64gc-unknown-linux-musl".into(),
        other => format!("{other}-unknown-linux-musl"),
    }
}

pub fn host_triple() -> Option<String> {
    let out = Command::new("rustc").args(["-vV"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(host) = line.strip_prefix("host: ") {
            return Some(host.trim().to_string());
        }
    }
    None
}

fn cargo_build(root: &Path, triple: &str, profile: &str, quiet: bool) -> Result<(), String> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root)
        .args(["build", "-p", AGENT_CRATE, "--bin", AGENT_BIN])
        .arg("--target")
        .arg(triple);
    if profile == "release" {
        cmd.arg("--release");
    }
    run_cargo(&mut cmd, quiet)
}

fn configure_cargo_stdio(cmd: &mut Command, quiet: bool) {
    if quiet {
        cmd.stdout(Stdio::null()).stderr(Stdio::piped());
    }
}

fn run_cargo(cmd: &mut Command, quiet: bool) -> Result<(), String> {
    configure_cargo_stdio(cmd, quiet);
    if quiet {
        let output = cmd.output().map_err(|e| e.to_string())?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.trim().is_empty() {
            "cargo build failed".into()
        } else {
            format!("cargo build failed:\n{stderr}")
        });
    }
    let status = cmd.status().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("cargo build failed".into());
    }
    Ok(())
}

/// Default path used by push transport: host agent in `target/infrazeug-agents/<host>/`.
pub fn agent_path_for_triple(workspace_root: impl AsRef<Path>, triple: &str) -> PathBuf {
    workspace_root
        .as_ref()
        .join("target/infrazeug-agents")
        .join(triple)
        .join(AGENT_BIN)
}

/// Default agent for the controller host triple.
pub fn default_agent_path(workspace_root: impl AsRef<Path>) -> PathBuf {
    let triple = host_triple()
        .map(|t| normalize_target_triple(&t))
        .unwrap_or_else(|| "host".to_string());
    agent_path_for_triple(workspace_root, &triple)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_bare_uname() {
        assert_eq!(
            normalize_target_triple("x86_64"),
            "x86_64-unknown-linux-musl"
        );
        assert_eq!(
            normalize_target_triple("aarch64"),
            "aarch64-unknown-linux-musl"
        );
    }

    #[test]
    fn preserves_full_triple() {
        assert_eq!(
            normalize_target_triple("x86_64-unknown-linux-gnu"),
            "x86_64-unknown-linux-gnu"
        );
    }
}
