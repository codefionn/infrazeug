//! qcow2 overlay helpers for stack VMs.

use std::path::Path;
use tokio::process::Command;

pub async fn create_overlay(base: &Path, overlay: &Path) -> Result<(), String> {
    create_overlay_sized(base, overlay, None).await
}

/// Create a qcow2 overlay over `base`. When `size_gb` is set and exceeds the base
/// image's virtual size, the overlay disk is grown to that size — the Debian cloud
/// image's cloud-init growpart/resizefs then expands the root fs to fill it on boot.
/// Needed for the k3s stack, whose container images far exceed the 3 GiB base disk.
pub async fn create_overlay_sized(
    base: &Path,
    overlay: &Path,
    size_gb: Option<u32>,
) -> Result<(), String> {
    if let Some(parent) = overlay.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    let mut cmd = Command::new("qemu-img");
    cmd.args([
        "create",
        "-f",
        "qcow2",
        "-F",
        "qcow2",
        "-b",
        base.to_str().ok_or("base path")?,
        overlay.to_str().ok_or("overlay path")?,
    ]);
    if let Some(gb) = size_gb {
        cmd.arg(format!("{gb}G"));
    }
    let status = cmd.status().await.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("qemu-img create overlay failed".into())
    }
}

pub fn qemu_img_available() -> bool {
    std::process::Command::new("qemu-img")
        .arg("--version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
