//! Build, prepare agents, and exec the native playbook binary.

use crate::arch_probe::probe_uname_machine;
use crate::discover::PlaybookProject;
use crate::PROBE_SUBCOMMAND;
use anyhow::Context;
use infrazeug_api::probe::ProbeExport;
use infrazeug_build::{build_agent, host_triple, uname_machine_to_triple, AgentBuildOptions};
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

pub fn is_playbook_subcommand(name: &str) -> bool {
    // Canonical list lives in infrazeug-api so new subcommands (e.g. `graph`)
    // are forwarded to the discovered playbook binary without drift here.
    infrazeug_api::cli::PLAYBOOK_SUBCOMMANDS.contains(&name)
}

/// Playbook or MCP subcommands that the stock CLI forwards via native rebuild + exec.
pub fn is_forwarded_subcommand(name: &str) -> bool {
    is_playbook_subcommand(name) || infrazeug_api::MCP_SUBCOMMANDS.contains(&name)
}

/// Native binary path under a Cargo `target_directory`.
pub fn playbook_binary_path(target_dir: &Path, bin_name: &str, release: bool) -> PathBuf {
    let profile = if release { "release" } else { "debug" };
    target_dir.join(profile).join(bin_name)
}

/// Native debug binary path after `cargo build` (package-local `target/` only).
#[allow(dead_code)]
pub fn native_binary_path(project: &PlaybookProject, release: bool) -> PathBuf {
    playbook_binary_path(
        &project.manifest_dir.join("target"),
        &project.bin_name,
        release,
    )
}

async fn cargo_target_directory(manifest_dir: &Path) -> anyhow::Result<PathBuf> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(manifest_dir)
        .output()
        .await
        .context("cargo metadata")?;
    if !output.status.success() {
        anyhow::bail!("cargo metadata failed");
    }
    let meta: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parse metadata")?;
    let dir = meta
        .get("target_directory")
        .and_then(|v| v.as_str())
        .context("metadata missing target_directory")?;
    Ok(PathBuf::from(dir))
}

pub fn release_profile_from_env() -> bool {
    std::env::var("INFRAZEUG_RELEASE")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

pub async fn build_playbook_native(
    project: &PlaybookProject,
    release: bool,
) -> anyhow::Result<PathBuf> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&project.manifest_dir)
        .args(["build", "--bin", &project.bin_name]);
    if release {
        cmd.arg("--release");
    }
    let status = cmd.status().await.context("cargo build playbook")?;
    if !status.success() {
        anyhow::bail!("cargo build --bin {} failed", project.bin_name);
    }
    let target_dir = cargo_target_directory(&project.manifest_dir).await?;
    let bin = playbook_binary_path(&target_dir, &project.bin_name, release);
    if !bin.is_file() {
        anyhow::bail!("playbook binary missing at {}", bin.display());
    }
    Ok(bin)
}

/// Build the playbook binary and prepare agents (probe + cross-build when needed).
pub async fn prepare_playbook(project: &PlaybookProject) -> anyhow::Result<PathBuf> {
    let release = release_profile_from_env();
    let binary = build_playbook_native(project, release).await?;
    let export = run_playbook_probe(&binary).await?;
    prepare_agents_for_export(&export, release).await?;
    Ok(binary)
}

/// Cross-build (or host-build) the agents named by a probe export.
///
/// This is the step that may reach out over SSH (`probe_uname_machine`) and can
/// therefore hang or fail when hosts are unreachable. It is split out of
/// [`prepare_playbook`] so `mcp serve` watch mode can run it *after* it has
/// already served the offline planning graph, and tolerate its failure.
pub async fn prepare_agents_for_export(export: &ProbeExport, release: bool) -> anyhow::Result<()> {
    let agent_ws = infrazeug_build::infrazeug_workspace_root();
    if !export.remotes.is_empty() || export.has_native_nodes {
        ensure_agents_for_probe(&agent_ws, export, release).await
    } else {
        let host = host_triple().unwrap_or_else(|| export.host_triple.clone());
        build_agent(
            &agent_ws,
            &AgentBuildOptions {
                targets: vec![host],
                release,
                quiet: false,
            },
        )
        .map(|_| ())
        .map_err(anyhow::Error::msg)
    }
}

pub async fn run_playbook_probe(binary: &Path) -> anyhow::Result<ProbeExport> {
    let out = Command::new(binary)
        .arg(PROBE_SUBCOMMAND)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .await
        .with_context(|| format!("run {} {}", binary.display(), PROBE_SUBCOMMAND))?;
    if !out.status.success() {
        anyhow::bail!("{} {} failed", binary.display(), PROBE_SUBCOMMAND);
    }
    serde_json::from_slice(&out.stdout).context("parse probe JSON")
}

pub async fn ensure_agents_for_probe(
    agent_workspace: &Path,
    export: &ProbeExport,
    release: bool,
) -> anyhow::Result<()> {
    let mut triples = HashSet::new();
    triples.insert(export.host_triple.clone());

    for remote in &export.remotes {
        let uname = if let Some(ref arch) = remote.os_arch {
            arch.clone()
        } else {
            probe_uname_machine(&remote.ssh, None).await?
        };
        triples.insert(uname_machine_to_triple(&uname));
    }

    info!(?triples, "building infrazeug-agent for target triples");
    let opts = AgentBuildOptions {
        targets: triples.into_iter().collect(),
        release,
        quiet: false,
    };
    build_agent(agent_workspace, &opts).map_err(anyhow::Error::msg)?;
    Ok(())
}

/// Build native playbook, prepare agents, exec with forwarded argv (skip `infrazeug`).
pub async fn run_playbook_command(
    project: &PlaybookProject,
    forwarded_argv: impl IntoIterator<Item = impl Into<OsString>>,
) -> anyhow::Result<()> {
    let forwarded_argv: Vec<OsString> = forwarded_argv.into_iter().map(Into::into).collect();
    let binary = prepare_playbook(project).await?;

    let status = Command::new(&binary)
        .args(forwarded_argv)
        .status()
        .await
        .context("exec playbook")?;
    if !status.success() {
        anyhow::bail!("playbook exited with {status}");
    }
    Ok(())
}
