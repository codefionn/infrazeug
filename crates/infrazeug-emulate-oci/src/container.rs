//! Podman/Docker CLI wrapper (podman preferred).

use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, warn};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OciRuntimeKind {
    Podman,
    Docker,
}

#[derive(Clone, Debug)]
pub struct ContainerCli {
    pub bin: String,
    pub kind: OciRuntimeKind,
}

impl Default for ContainerCli {
    fn default() -> Self {
        Self {
            bin: std::env::var("INFRZEUG_PODMAN").unwrap_or_else(|_| "podman".into()),
            kind: OciRuntimeKind::Podman,
        }
    }
}

impl ContainerCli {
    pub fn new(bin: impl Into<String>, kind: OciRuntimeKind) -> Self {
        Self {
            bin: bin.into(),
            kind,
        }
    }

    pub fn runtime_name(&self) -> &'static str {
        match self.kind {
            OciRuntimeKind::Podman => "podman",
            OciRuntimeKind::Docker => "docker",
        }
    }

    pub async fn available(&self) -> bool {
        Command::new(&self.bin)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub async fn build(
        &self,
        context: &Path,
        containerfile: &Path,
        tag: &str,
    ) -> Result<(), String> {
        let status = Command::new(&self.bin)
            .args([
                "build",
                "-f",
                containerfile.to_str().ok_or("bad path")?,
                "-t",
                tag,
                context.to_str().ok_or("bad context")?,
            ])
            .status()
            .await
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{} build failed for {tag}", self.runtime_name()))
        }
    }

    pub async fn run_detached(
        &self,
        image: &str,
        name: &str,
        label_run_id: &str,
    ) -> Result<(), String> {
        self.run_detached_with(image, name, label_run_id, &[], &[], None)
            .await
    }

    pub async fn run_detached_with(
        &self,
        image: &str,
        name: &str,
        label_run_id: &str,
        env: &[(&str, &str)],
        extra_args: &[&str],
        command: Option<&[&str]>,
    ) -> Result<(), String> {
        let mut cmd = Command::new(&self.bin);
        cmd.args(["run", "-d", "--name", name]);
        cmd.args(["--label", &format!("infrazeug.run_id={label_run_id}")]);
        for (k, v) in env {
            cmd.args(["-e", &format!("{k}={v}")]);
        }
        for arg in extra_args {
            cmd.arg(arg);
        }
        cmd.arg(image);
        if let Some(argv) = command {
            for a in argv {
                cmd.arg(a);
            }
        } else {
            cmd.args(["sh", "-c", "sleep infinity"]);
        }
        let status = cmd.status().await.map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{} run failed for {name}", self.runtime_name()))
        }
    }

    pub async fn network_create(&self, name: &str) -> Result<(), String> {
        let status = Command::new(&self.bin)
            .args(["network", "create", name])
            .status()
            .await
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!(
                "{} network create failed for {name}",
                self.runtime_name()
            ))
        }
    }

    pub async fn network_rm(&self, name: &str) -> Result<(), String> {
        let _ = Command::new(&self.bin)
            .args(["network", "rm", name])
            .status()
            .await;
        Ok(())
    }

    pub async fn rm_force(&self, name: &str) -> Result<(), String> {
        let _ = Command::new(&self.bin)
            .args(["rm", "-f", name])
            .status()
            .await;
        Ok(())
    }

    pub async fn image_exists(&self, tag: &str) -> bool {
        Command::new(&self.bin)
            .args(["image", "exists", tag])
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub async fn container_running(&self, name: &str) -> bool {
        let out = Command::new(&self.bin)
            .args(["inspect", "-f", "{{.State.Running}}", name])
            .output()
            .await
            .ok();
        out.map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
            .unwrap_or(false)
    }

    pub async fn exec(&self, container: &str, argv: &[&str]) -> Result<i32, String> {
        let mut cmd = Command::new(&self.bin);
        cmd.arg("exec").arg(container);
        for a in argv {
            cmd.arg(a);
        }
        let out = cmd.output().await.map_err(|e| e.to_string())?;
        Ok(out.status.code().unwrap_or(-1))
    }
}

/// Resolve a working OCI CLI: explicit `INFRZEUG_CONTAINER_RUNTIME`, then podman, then docker.
pub async fn resolve_container_cli() -> Option<ContainerCli> {
    if let Ok(bin) = std::env::var("INFRZEUG_CONTAINER_RUNTIME") {
        let kind = if bin.contains("docker") {
            OciRuntimeKind::Docker
        } else {
            OciRuntimeKind::Podman
        };
        let cli = ContainerCli::new(bin, kind);
        if cli.available().await {
            return Some(cli);
        }
        return None;
    }

    if let Ok(bin) = std::env::var("INFRZEUG_PODMAN") {
        let cli = ContainerCli::new(bin, OciRuntimeKind::Podman);
        if cli.available().await {
            return Some(cli);
        }
    } else {
        let cli = ContainerCli::new("podman", OciRuntimeKind::Podman);
        if cli.available().await {
            return Some(cli);
        }
    }

    if let Ok(bin) = std::env::var("INFRZEUG_DOCKER") {
        let cli = ContainerCli::new(bin, OciRuntimeKind::Docker);
        if cli.available().await {
            return Some(cli);
        }
    } else {
        let cli = ContainerCli::new("docker", OciRuntimeKind::Docker);
        if cli.available().await {
            return Some(cli);
        }
    }

    None
}

pub fn warn_if_missing_runtime(available: bool, runtime: &str) {
    if !available {
        warn!("{runtime} not found; container emulation will fail at runtime");
    } else {
        debug!("{runtime} available for OCI emulation");
    }
}

/// Backward-compatible alias.
pub type PodmanCli = ContainerCli;
