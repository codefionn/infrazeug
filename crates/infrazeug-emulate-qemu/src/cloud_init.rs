//! NoCloud seed ISO for first-boot SSH.

use infrazeug_emulate::spec::{QemuConfig, VmImage};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;

#[derive(Clone, Debug)]
pub struct SshGuestConfig {
    pub user: String,
    pub ssh_pubkey: String,
}

impl Default for SshGuestConfig {
    fn default() -> Self {
        Self {
            user: "infrazeug".into(),
            ssh_pubkey: String::new(),
        }
    }
}

/// Cloud-init payload for a stack member (dual-NIC: DHCP + static internal).
#[derive(Clone, Debug)]
pub struct StackGuestConfig {
    pub ssh: SshGuestConfig,
    pub hostname: String,
    /// e.g. `192.168.100.10/24` on the internal virtio NIC (`enp0s3`).
    pub internal_address: String,
    /// Extra `/etc/hosts` lines (stack peer names).
    pub hosts_table: String,
}

pub struct CloudInitSeed {
    pub workspace: PathBuf,
}

impl CloudInitSeed {
    pub async fn write(&self, guest: &SshGuestConfig) -> Result<PathBuf, String> {
        let cidata = self.workspace.join("cidata");
        fs::create_dir_all(&cidata)
            .await
            .map_err(|e| e.to_string())?;
        // A unique instance-id per boot so cloud-init always re-runs its per-instance
        // modules (user + SSH key injection). A fixed id would make repeated boots
        // from the same/overlaid image skip key setup as "already provisioned".
        let meta = format!(
            "instance-id: infrazeug-{}\nlocal-hostname: infrazeug\n",
            uuid::Uuid::new_v4()
        );
        fs::write(cidata.join("meta-data"), meta)
            .await
            .map_err(|e| e.to_string())?;
        // Keep this in step with `write_lab_guest`: include the image's default
        // user, do not run apt on first boot (the Debian cloud image already ships
        // sshd, and `apt-get update` over slirp can stall key setup).
        let user_data = format!(
            r#"#cloud-config
users:
  - default
  - name: {user}
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    ssh_authorized_keys:
      - {key}
ssh_pwauth: false
package_update: false
runcmd:
  - systemctl enable --now ssh
"#,
            user = guest.user,
            key = guest.ssh_pubkey.trim()
        );
        fs::write(cidata.join("user-data"), user_data)
            .await
            .map_err(|e| e.to_string())?;
        let iso = self.workspace.join("seed.iso");
        build_seed_iso(&cidata, &iso).await?;
        Ok(iso)
    }

    /// Minimal first-boot seed for k3s lab VMs (SSH only; packages installed in bootstrap).
    pub async fn write_lab_guest(
        &self,
        guest: &SshGuestConfig,
        hostname: &str,
    ) -> Result<PathBuf, String> {
        let cidata = self.workspace.join("cidata");
        fs::create_dir_all(&cidata)
            .await
            .map_err(|e| e.to_string())?;
        let meta = format!("instance-id: infrazeug-{hostname}\nlocal-hostname: {hostname}\n");
        fs::write(cidata.join("meta-data"), meta)
            .await
            .map_err(|e| e.to_string())?;
        let user_data = format!(
            r#"#cloud-config
hostname: {hostname}
users:
  - default
  - name: {user}
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    ssh_authorized_keys:
      - {key}
ssh_pwauth: false
package_update: false
runcmd:
  - systemctl enable --now ssh
"#,
            hostname = hostname,
            user = guest.user,
            key = guest.ssh_pubkey.trim()
        );
        fs::write(cidata.join("user-data"), user_data)
            .await
            .map_err(|e| e.to_string())?;
        let iso = self.workspace.join("seed.iso");
        build_seed_iso(&cidata, &iso).await?;
        Ok(iso)
    }

    pub async fn write_stack_guest(&self, guest: &StackGuestConfig) -> Result<PathBuf, String> {
        let cidata = self.workspace.join("cidata");
        fs::create_dir_all(&cidata)
            .await
            .map_err(|e| e.to_string())?;
        let meta = format!(
            "instance-id: infrazeug-{}\nlocal-hostname: {}\n",
            guest.hostname, guest.hostname
        );
        fs::write(cidata.join("meta-data"), meta)
            .await
            .map_err(|e| e.to_string())?;
        let user_data = format!(
            r#"#cloud-config
hostname: {hostname}
manage_etc_hosts: true
users:
  - default
  - name: {user}
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    ssh_authorized_keys:
      - {key}
ssh_pwauth: false
package_update: false
packages:
  - iputils-ping
  - curl
write_files:
  - path: /etc/hosts
    append: true
    content: |
{hosts}
runcmd:
  - |
    for d in enp0s4 ens4 enp0s5 eth1; do
      ip link set "$d" up 2>/dev/null || continue
      ip addr add {lan_ip}/24 dev "$d" 2>/dev/null && exit 0
    done
    exit 1
"#,
            hostname = guest.hostname,
            user = guest.ssh.user,
            key = guest.ssh.ssh_pubkey.trim(),
            hosts = indent_hosts(&guest.hosts_table),
            lan_ip = guest.internal_address.trim_end_matches("/24"),
        );
        fs::write(cidata.join("user-data"), user_data)
            .await
            .map_err(|e| e.to_string())?;
        let iso = self.workspace.join("seed.iso");
        build_seed_iso(&cidata, &iso).await?;
        Ok(iso)
    }
}

fn indent_hosts(table: &str) -> String {
    table
        .lines()
        .map(|l| format!("      {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn build_seed_iso(cidata: &Path, iso: &Path) -> Result<(), String> {
    if Command::new("genisoimage")
        .args([
            "-output",
            iso.to_str().ok_or("iso path")?,
            "-volid",
            "cidata",
            "-joliet",
            "-rock",
            cidata.to_str().ok_or("cidata path")?,
        ])
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Ok(());
    }
    if Command::new("mkisofs")
        .args([
            "-output",
            iso.to_str().ok_or("iso path")?,
            "-volid",
            "cidata",
            "-joliet",
            "-rock",
            cidata.to_str().ok_or("cidata path")?,
        ])
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Ok(());
    }
    build_seed_iso_via_container(cidata, iso).await
}

async fn build_seed_iso_via_container(cidata: &Path, iso: &Path) -> Result<(), String> {
    let runtime = resolve_iso_runtime();
    let cidata = cidata.canonicalize().map_err(|e| e.to_string())?;
    let out_dir = iso
        .parent()
        .ok_or("iso parent")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let iso_name = iso
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("iso filename")?;
    let script = format!(
        "apt-get update -qq && apt-get install -y -qq genisoimage >/dev/null && \
         genisoimage -output /out/{iso_name} -volid cidata -joliet -rock /cidata"
    );
    let status = Command::new(&runtime)
        .args([
            "run",
            "--rm",
            &format!("-v{}:/cidata:Z", cidata.display()),
            &format!("-v{}:/out:Z", out_dir.display()),
            "docker.io/library/debian:bookworm-slim",
            "bash",
            "-c",
            &script,
        ])
        .status()
        .await
        .map_err(|e| e.to_string())?;
    if status.success() && iso.exists() {
        Ok(())
    } else {
        Err(format!(
            "{runtime} could not build cloud-init seed ISO (install genisoimage/mkisofs locally)"
        ))
    }
}

fn resolve_iso_runtime() -> String {
    if let Ok(bin) = std::env::var("INFRZEUG_PODMAN") {
        return bin;
    }
    if std::process::Command::new("podman")
        .arg("--version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return "podman".into();
    }
    std::env::var("INFRZEUG_DOCKER").unwrap_or_else(|_| "docker".into())
}

pub fn resolve_image(image: &VmImage) -> Result<PathBuf, String> {
    match image {
        VmImage::RemoteQcow2(path) => {
            let p = PathBuf::from(path);
            if p.exists() {
                Ok(p)
            } else {
                Err(format!("qcow2 image not found: {}", p.display()))
            }
        }
    }
}

pub fn memory_mb(config: &QemuConfig) -> u32 {
    config.memory_mb.max(256)
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_emulate::spec::{QemuConfig, VmImage};
    use std::path::PathBuf;

    #[test]
    fn indent_hosts_adds_padding() {
        let out = indent_hosts("10.0.0.1 db\n10.0.0.2 idp");
        assert!(out.contains("      10.0.0.1 db"));
        assert!(out.contains("      10.0.0.2 idp"));
    }

    #[test]
    fn memory_mb_floor() {
        assert_eq!(memory_mb(&QemuConfig { memory_mb: 64 }), 256);
        assert_eq!(memory_mb(&QemuConfig { memory_mb: 1024 }), 1024);
    }

    #[test]
    fn resolve_image_missing_file() {
        let err = resolve_image(&VmImage::RemoteQcow2("/no/such/image.qcow2".into())).unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn resolve_image_existing_file() {
        let p = std::env::temp_dir().join(format!("iz-qemu-{}.qcow2", uuid::Uuid::new_v4()));
        std::fs::write(&p, b"").unwrap();
        let got = resolve_image(&VmImage::RemoteQcow2(p.display().to_string())).unwrap();
        assert_eq!(got, PathBuf::from(&p));
        let _ = std::fs::remove_file(&p);
    }
}
