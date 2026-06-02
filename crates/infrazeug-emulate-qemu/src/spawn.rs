//! QEMU process invocation.

use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QemuArch {
    X86_64,
    Aarch64,
}

impl QemuArch {
    pub fn detect() -> Self {
        if std::env::consts::ARCH == "aarch64" {
            Self::Aarch64
        } else {
            Self::X86_64
        }
    }

    pub fn binary(self) -> &'static str {
        match self {
            Self::X86_64 => "qemu-system-x86_64",
            Self::Aarch64 => "qemu-system-aarch64",
        }
    }
}

/// vCPUs per guest (override with `INFRZEUG_QEMU_CPUS`). Default 2 keeps the small
/// 4-VM stack light; the k3s stack sets a higher value for faster image work.
fn qemu_cpus() -> u32 {
    std::env::var("INFRZEUG_QEMU_CPUS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n >= 1)
        .unwrap_or(2)
}

pub fn qemu_available() -> bool {
    let arch = QemuArch::detect();
    std::process::Command::new("which")
        .arg(arch.binary())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub struct QemuSpawn {
    pub child: Child,
    pub ssh_port: u16,
}

pub async fn spawn_qemu(
    arch: QemuArch,
    qcow2: &Path,
    seed_iso: &Path,
    memory_mb: u32,
    ssh_port: u16,
    accel: bool,
) -> Result<QemuSpawn, String> {
    let mut cmd = Command::new(arch.binary());
    if accel {
        cmd.arg("-machine").arg("q35,accel=kvm");
        cmd.arg("-cpu").arg("host");
    } else {
        cmd.arg("-machine").arg("q35,accel=tcg");
        cmd.arg("-cpu").arg("qemu64");
    }
    cmd.arg("-m").arg(memory_mb.to_string());
    cmd.arg("-smp").arg(qemu_cpus().to_string());
    cmd.arg("-drive")
        .arg(format!("file={},format=qcow2,if=virtio", qcow2.display()));
    cmd.arg("-drive").arg(format!(
        "file={},format=raw,if=virtio,readonly=on",
        seed_iso.display()
    ));
    cmd.arg("-netdev")
        .arg(format!("user,id=net0,hostfwd=tcp:127.0.0.1:{ssh_port}-:22"));
    cmd.arg("-device").arg("virtio-net-pci,netdev=net0");
    cmd.arg("-display").arg("none");
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    let child = cmd.spawn().map_err(|e| format!("spawn qemu: {e}"))?;
    Ok(QemuSpawn { child, ssh_port })
}

/// User netdev (SSH hostfwd) + multicast socket NIC (inter-VM on separate QEMU processes).
#[cfg(test)]
pub async fn spawn_qemu_stack_member(
    arch: QemuArch,
    qcow2: &Path,
    seed_iso: &Path,
    memory_mb: u32,
    ssh_port: u16,
    mcast: &str,
    accel: bool,
) -> Result<QemuSpawn, String> {
    let mut cmd = Command::new(arch.binary());
    if accel {
        cmd.arg("-machine").arg("q35,accel=kvm");
        cmd.arg("-cpu").arg("host");
    } else {
        cmd.arg("-machine").arg("q35,accel=tcg");
        cmd.arg("-cpu").arg("qemu64");
    }
    cmd.arg("-m").arg(memory_mb.to_string());
    cmd.arg("-drive")
        .arg(format!("file={},format=qcow2,if=virtio", qcow2.display()));
    cmd.arg("-drive").arg(format!(
        "file={},format=raw,if=virtio,readonly=on",
        seed_iso.display()
    ));
    cmd.arg("-netdev")
        .arg(format!("user,id=ssh0,hostfwd=tcp:127.0.0.1:{ssh_port}-:22"));
    cmd.arg("-device")
        .arg("virtio-net-pci,netdev=ssh0,addr=0x3");
    cmd.arg("-netdev")
        .arg(format!("socket,id=lan0,mcast={mcast}"));
    cmd.arg("-device")
        .arg("virtio-net-pci,netdev=lan0,addr=0x4");
    cmd.arg("-display").arg("none");
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    let child = cmd.spawn().map_err(|e| format!("spawn qemu: {e}"))?;
    Ok(QemuSpawn { child, ssh_port })
}

pub async fn wait_ssh_port(port: u16, timeout_secs: u64) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
    while tokio::time::Instant::now() < deadline {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return Ok(());
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    Err(format!("ssh port {port} not ready within {timeout_secs}s"))
}
