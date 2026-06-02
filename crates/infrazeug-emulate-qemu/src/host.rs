//! Boot/teardown helpers for MicroVm `like` targets.

use crate::cloud_init::{memory_mb, resolve_image, CloudInitSeed, SshGuestConfig};
use crate::spawn::{qemu_available, spawn_qemu, wait_ssh_port, QemuArch, QemuSpawn};
use infrazeug_core::machine::SshConfig;
use infrazeug_emulate::spec::{QemuConfig, VmImage};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct MicroVmHandle {
    pub name: String,
    pub ssh_port: u16,
    pub pid_file: PathBuf,
}

#[derive(Clone, Debug)]
pub struct MicroVmBoot {
    pub ssh: SshConfig,
    pub handle: MicroVmHandle,
}

pub struct QemuHost {
    pub run_workspace: PathBuf,
}

impl QemuHost {
    pub fn new(run_workspace: PathBuf) -> Self {
        Self { run_workspace }
    }
}

pub async fn boot_microvm(
    host: &QemuHost,
    run_id: Uuid,
    machine_name: &str,
    image: &VmImage,
    qemu: &QemuConfig,
    guest: &SshGuestConfig,
) -> Result<MicroVmBoot, String> {
    if !qemu_available() {
        return Err("qemu-system-* not found in PATH".into());
    }
    let qcow2 = resolve_image(image)?;
    let ws = host
        .run_workspace
        .join(run_id.to_string())
        .join("qemu")
        .join(sanitize_name(machine_name));
    tokio::fs::create_dir_all(&ws)
        .await
        .map_err(|e| e.to_string())?;

    let seed = CloudInitSeed {
        workspace: ws.clone(),
    };
    let seed_iso = seed.write(guest).await?;

    let ssh_port = pick_port(&ws).await?;
    let arch = QemuArch::detect();
    let kvm = std::path::Path::new("/dev/kvm").exists();
    let QemuSpawn {
        mut child,
        ssh_port,
    } = spawn_qemu(arch, &qcow2, &seed_iso, memory_mb(qemu), ssh_port, kvm).await?;

    let pid = child.id().unwrap_or(0);
    tokio::fs::write(ws.join("qemu.pid"), pid.to_string())
        .await
        .map_err(|e| e.to_string())?;

    if wait_ssh_port(ssh_port, 120).await.is_err() {
        let _ = child.kill().await;
        return Err("guest SSH did not become ready".into());
    }

    tokio::spawn(async move {
        let _ = child.wait().await;
    });

    let ssh = SshConfig::new(format!("127.0.0.1:{ssh_port}")).with_user(&guest.user);
    let handle = MicroVmHandle {
        name: machine_name.to_string(),
        ssh_port,
        pid_file: ws.join("qemu.pid"),
    };
    Ok(MicroVmBoot { ssh, handle })
}

pub async fn stop_microvm(handle: &MicroVmHandle) -> Result<(), String> {
    if let Ok(pid_s) = tokio::fs::read_to_string(&handle.pid_file).await {
        if let Ok(pid) = pid_s.trim().parse::<u32>() {
            let _ = tokio::process::Command::new("kill")
                .arg(pid.to_string())
                .status()
                .await;
        }
    }
    Ok(())
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

async fn pick_port(ws: &Path) -> Result<u16, String> {
    let path = ws.join("ssh.port");
    if let Ok(s) = tokio::fs::read_to_string(&path).await {
        if let Ok(p) = s.trim().parse::<u16>() {
            return Ok(p);
        }
    }
    let port = 2222u16
        .checked_add((rand_port_offset() % 500) as u16)
        .unwrap_or(2222);
    tokio::fs::write(&path, port.to_string())
        .await
        .map_err(|e| e.to_string())?;
    Ok(port)
}

fn rand_port_offset() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}
