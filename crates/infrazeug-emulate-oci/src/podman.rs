//! Podman/Docker CLI integration (see [`crate::container`]).

pub use crate::container::{
    resolve_container_cli, warn_if_missing_runtime,
    warn_if_missing_runtime as warn_if_missing_podman, ContainerCli, OciRuntimeKind, PodmanCli,
};

use std::path::{Path, PathBuf};

pub fn build_context_dir(workspace: &Path, run_uuid: &str, spec_id: &str) -> PathBuf {
    workspace
        .join(".infrazeug")
        .join("build")
        .join(run_uuid)
        .join(spec_id)
}

pub async fn write_containerfile(dir: &Path, body: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join("Containerfile");
    std::fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(path)
}
