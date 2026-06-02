//! Debian cloud-image microVM stack on an internal multicast L2 (test-only).

use crate::cloud_init::{CloudInitSeed, SshGuestConfig, StackGuestConfig};
use crate::guest_ssh::{ssh_capture, ssh_run, wait_cloud_init};
use crate::host::{stop_microvm, MicroVmHandle, QemuHost};
use crate::overlay::{create_overlay, qemu_img_available};
use crate::spawn::{qemu_available, spawn_qemu_stack_member, wait_ssh_port, QemuArch, QemuSpawn};
use infrazeug_core::machine::SshConfig;
use infrazeug_emulate::spec::QemuConfig;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MCAST_BASE: &str = "230.0.0.100";
const STACK_SUBNET: &str = "192.168.100.0/24";
const STACK_LABEL: &str = "infrazeug.debian_vm_stack";
const AGENT_UPDATE_CHANGED: &str = "infrazeug-agent-update: changed";
const AGENT_UPDATE_UNCHANGED: &str = "infrazeug-agent-update: unchanged";

/// Default Debian 12 generic cloud image (override with `INFRZEUG_DEBIAN_CLOUD_IMAGE`).
pub const DEBIAN_12_CLOUD_AMD64: &str =
    "https://cdimage.debian.org/cdimage/cloud/bookworm/latest/debian-12-generic-amd64.qcow2";

struct StackNode {
    hostname: &'static str,
    handle: MicroVmHandle,
    ssh: SshConfig,
}

/// Four Debian bookworm VMs on a private multicast network (mirrors the OCI lab layout).
#[allow(dead_code)]
pub struct DebianVmStack {
    run_id: String,
    mcast: String,
    workspace: PathBuf,
    nodes: Vec<StackNode>,
    ssh_user: String,
    ssh_identity: Option<PathBuf>,
}

impl DebianVmStack {
    pub async fn up(
        host: &QemuHost,
        base_image: &Path,
        ssh_pubkey: &str,
        ssh_user: &str,
        ssh_identity: Option<PathBuf>,
    ) -> Result<Self, String> {
        if !qemu_available() {
            return Err("qemu-system-* not found in PATH".into());
        }
        if !qemu_img_available() {
            return Err("qemu-img not found in PATH".into());
        }
        if !base_image.exists() {
            return Err(format!(
                "debian cloud image not found: {} (set INFRZEUG_DEBIAN_CLOUD_IMAGE)",
                base_image.display()
            ));
        }

        let run_id = Uuid::new_v4().to_string();
        let short = &run_id[..8];
        let mcast = format!("{MCAST_BASE}:{}", 11100 + port_offset() % 200);
        let workspace = host.run_workspace.join(&run_id).join("debian-stack");
        tokio::fs::create_dir_all(&workspace)
            .await
            .map_err(|e| e.to_string())?;

        let guest_ssh = SshGuestConfig {
            user: ssh_user.into(),
            ssh_pubkey: ssh_pubkey.into(),
        };

        let members = [
            ("iz-db", "192.168.100.10"),
            ("iz-idp", "192.168.100.11"),
            ("iz-ui", "192.168.100.12"),
            ("iz-store", "192.168.100.13"),
        ];
        let hosts_table = members
            .iter()
            .map(|(name, ip)| format!("{ip} {name}"))
            .collect::<Vec<_>>()
            .join("\n");

        let arch = QemuArch::detect();
        let kvm = Path::new("/dev/kvm").exists();
        let qemu_cfg = QemuConfig {
            memory_mb: std::env::var("INFRZEUG_VM_STACK_MEM_MB")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(768),
        };

        let mut nodes = Vec::new();
        for (i, (hostname, guest_ip)) in members.iter().enumerate() {
            let vm_ws = workspace.join(hostname);
            tokio::fs::create_dir_all(&vm_ws)
                .await
                .map_err(|e| e.to_string())?;
            let overlay = vm_ws.join("disk.qcow2");
            create_overlay(base_image, &overlay).await?;
            let ssh_port = 3500 + (port_offset() % 500) as u16 + i as u16;

            let seed = CloudInitSeed {
                workspace: vm_ws.clone(),
            };
            let seed_iso = seed
                .write_stack_guest(&StackGuestConfig {
                    ssh: guest_ssh.clone(),
                    hostname: (*hostname).into(),
                    internal_address: format!("{guest_ip}/24"),
                    hosts_table: hosts_table.clone(),
                })
                .await?;

            let QemuSpawn {
                mut child,
                ssh_port: actual_port,
            } = spawn_qemu_stack_member(
                arch,
                &overlay,
                &seed_iso,
                qemu_cfg.memory_mb.max(512),
                ssh_port,
                &mcast,
                kvm,
            )
            .await?;

            let pid = child.id().unwrap_or(0);
            tokio::fs::write(vm_ws.join("qemu.pid"), pid.to_string())
                .await
                .map_err(|e| e.to_string())?;
            tokio::fs::write(vm_ws.join("stack.label"), STACK_LABEL)
                .await
                .map_err(|e| e.to_string())?;

            if wait_ssh_port(actual_port, 180).await.is_err() {
                let _ = child.kill().await;
                return Err(format!("SSH not ready on {hostname} (port {actual_port})"));
            }

            let ssh = SshConfig::new(format!("127.0.0.1:{actual_port}")).with_user(ssh_user);
            if wait_cloud_init(&ssh, ssh_identity.as_deref(), 300)
                .await
                .is_err()
            {
                let _ = child.kill().await;
                return Err(format!("cloud-init did not finish on {hostname}"));
            }
            if wait_lan_address(&ssh, ssh_identity.as_deref(), guest_ip, 120)
                .await
                .is_err()
            {
                let _ = child.kill().await;
                return Err(format!("internal LAN address not configured on {hostname}"));
            }

            tokio::spawn(async move {
                let _ = child.wait().await;
            });
            let handle = MicroVmHandle {
                name: format!("{hostname}-{short}"),
                ssh_port: actual_port,
                pid_file: vm_ws.join("qemu.pid"),
            };
            nodes.push(StackNode {
                hostname,
                handle,
                ssh,
            });
        }

        Ok(Self {
            run_id,
            mcast,
            workspace,
            nodes,
            ssh_user: ssh_user.into(),
            ssh_identity,
        })
    }

    pub fn mcast(&self) -> &str {
        &self.mcast
    }

    pub fn subnet(&self) -> &'static str {
        STACK_SUBNET
    }

    /// Each guest reports a Debian OS ID.
    pub async fn verify_debian(&self) -> Result<(), String> {
        for node in &self.nodes {
            let code = ssh_run(
                &node.ssh,
                self.ssh_identity.as_deref(),
                &["grep", "-qi", "debian", "/etc/os-release"],
            )
            .await?;
            if code != 0 {
                let hint = ssh_run(
                    &node.ssh,
                    self.ssh_identity.as_deref(),
                    &["cat", "/etc/os-release"],
                )
                .await
                .unwrap_or(-1);
                return Err(format!(
                    "{} os-release check failed (exit {code}, cat exit {hint})",
                    node.hostname
                ));
            }
        }
        Ok(())
    }

    /// Each guest has its static address on the internal NIC (`enp0s4`).
    ///
    /// Note: separate QEMU processes use multicast socket NICs; cross-VM L2 may not
    /// pass traffic on all hosts without a bridge (SOUL run netns). We verify
    /// address assignment and `/etc/hosts` peer entries instead.
    pub async fn verify_internal_lan(&self) -> Result<(), String> {
        let peers = [
            ("iz-db", "192.168.100.10"),
            ("iz-idp", "192.168.100.11"),
            ("iz-ui", "192.168.100.12"),
            ("iz-store", "192.168.100.13"),
        ];
        for (hostname, ip) in peers {
            let node = self
                .nodes
                .iter()
                .find(|n| n.hostname == hostname)
                .ok_or_else(|| format!("missing {hostname} node"))?;
            let script = format!("ip -4 addr | grep -q '{ip}/24' || ip -4 addr | grep -q '{ip}'");
            let code = ssh_run(
                &node.ssh,
                self.ssh_identity.as_deref(),
                &["sh", "-c", &script],
            )
            .await?;
            if code != 0 {
                return Err(format!(
                    "{hostname} internal LAN check failed (exit {code})"
                ));
            }
        }
        Ok(())
    }

    /// Apply Debian package updates on each guest agent.
    ///
    /// The dry-run pass gives us a stable idempotence signal: any planned
    /// install/removal means the node changed, while an empty plan is unchanged.
    pub async fn update_agents(&self) -> Result<Vec<AgentUpdate>, String> {
        let mut updates = Vec::new();
        for node in &self.nodes {
            let (code, output) =
                ssh_capture(&node.ssh, self.ssh_identity.as_deref(), AGENT_UPDATE_SCRIPT).await?;
            if code != 0 {
                return Err(format!(
                    "{} agent update failed (exit {code}):\n{output}",
                    node.hostname
                ));
            }
            let changed = output.contains(AGENT_UPDATE_CHANGED);
            let unchanged = output.contains(AGENT_UPDATE_UNCHANGED);
            if !changed && !unchanged {
                return Err(format!(
                    "{} agent update produced no change marker:\n{output}",
                    node.hostname
                ));
            }
            updates.push(AgentUpdate {
                hostname: node.hostname,
                changed,
            });
        }
        Ok(updates)
    }

    /// Reboot only guests whose update pass actually changed packages.
    pub async fn reboot_after_updates(&self, updates: &[AgentUpdate]) -> Result<(), String> {
        let changed_nodes = updates
            .iter()
            .filter(|u| u.changed)
            .map(|update| {
                self.nodes
                    .iter()
                    .find(|n| n.hostname == update.hostname)
                    .ok_or_else(|| format!("missing {} node", update.hostname))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.apply_expect_shutdown_reboots(&changed_nodes).await
    }

    pub async fn down(self) -> Result<(), String> {
        for node in &self.nodes {
            let _ = stop_microvm(&node.handle).await;
        }
        let _ = tokio::fs::remove_dir_all(&self.workspace).await;
        Ok(())
    }

    async fn apply_expect_shutdown_reboots(&self, nodes: &[&StackNode]) -> Result<(), String> {
        if nodes.is_empty() {
            return Ok(());
        }

        use infrazeug_core::machine::Lifecycle;
        use infrazeug_core::SchedEvent;
        use infrazeug_core::{
            AutoDenyInteractor, Infra, Machine, MachineId, MachineKind, NodeBuilder, NodeId,
            RuntimeConfig, Targets, TransportChoice,
        };
        use infrazeug_shell::{argv, ShellOp};
        use infrazeug_transport::TransportFactory;
        use std::collections::HashMap;
        use std::sync::Arc;
        use tokio::sync::{broadcast, mpsc};
        use tokio_util::sync::CancellationToken;

        let short = &self.run_id[..8];
        let short_run_root = std::env::temp_dir().join(format!("iz-qemu-{short}"));
        let mut infra = Infra::new()
            .with_default_remote_transport(TransportChoice::SshAgentless)
            .with_runtime(RuntimeConfig {
                run_root: short_run_root.join("runs"),
                vault_store: None,
            });

        for node in nodes {
            let machine_id = MachineId(Uuid::new_v4());
            let reboot_id = NodeId(Uuid::new_v4());

            infra = infra
                .add_machine(Machine {
                    id: machine_id,
                    name: format!("{}-expect-shutdown", node.hostname),
                    kind: MachineKind::Remote {
                        ssh: ssh_for_transport(&node.ssh, self.ssh_identity.as_deref())?,
                        os: None,
                    },
                    vars: Default::default(),
                    groups: Vec::new(),
                    tags: Vec::new(),
                    max_parallel_nodes: None,
                    lifecycle: Lifecycle::Persistent,
                    like: None,
                    lazy: false,
                })
                .map_err(|e| e.to_string())?;

            let reboot_node = NodeBuilder::shell(
                reboot_id,
                ShellOp::run(argv!["sh", "-c", EXPECT_SHUTDOWN_REBOOT_SCRIPT]),
                Targets::Machine(machine_id),
            )
            .name(format!("expect-shutdown-reboot@{}", node.hostname))
            .expect_shutdown(true)
            .build();

            infra = infra.add_node(reboot_node).map_err(|e| e.to_string())?;
        }

        let plan = infra.plan().map_err(|e| e.to_string())?;
        let node_names = infra
            .nodes
            .iter()
            .map(|node| (node.id, node.name.clone()))
            .collect::<HashMap<_, _>>();
        let machine_names = infra
            .machines
            .iter()
            .map(|machine| (machine.id, machine.name.clone()))
            .collect::<HashMap<_, _>>();
        let factory =
            TransportFactory::new(short_run_root.join("tx"), self.workspace.clone(), false);
        factory.prepare(&infra).await.map_err(|e| e.to_string())?;
        eprintln!(
            "running expect_shutdown reboot apply for: {}",
            nodes
                .iter()
                .map(|node| node.hostname)
                .collect::<Vec<_>>()
                .join(", ")
        );
        let (events, mut events_rx) = broadcast::channel(256);
        let event_task = tokio::spawn(async move {
            while let Ok(event) = events_rx.recv().await {
                match event {
                    SchedEvent::NodeStarted { node, machine } => {
                        eprintln!(
                            "expect_shutdown started: {} on {}",
                            node_names.get(&node).map(String::as_str).unwrap_or("?"),
                            machine_names
                                .get(&machine)
                                .map(String::as_str)
                                .unwrap_or("?")
                        );
                    }
                    SchedEvent::NodeProgress {
                        node,
                        machine,
                        message,
                    } => {
                        eprintln!(
                            "expect_shutdown progress: {} on {}: {}",
                            node_names.get(&node).map(String::as_str).unwrap_or("?"),
                            machine_names
                                .get(&machine)
                                .map(String::as_str)
                                .unwrap_or("?"),
                            message
                        );
                    }
                    SchedEvent::NodeOutput {
                        node,
                        machine,
                        stream,
                        data,
                    } => {
                        let text = String::from_utf8_lossy(&data);
                        let text = text.trim_end();
                        if !text.is_empty() {
                            eprintln!(
                                "expect_shutdown output {:?}: {} on {}: {}",
                                stream,
                                node_names.get(&node).map(String::as_str).unwrap_or("?"),
                                machine_names
                                    .get(&machine)
                                    .map(String::as_str)
                                    .unwrap_or("?"),
                                text
                            );
                        }
                    }
                    SchedEvent::NodeReconnecting {
                        node,
                        machine,
                        attempt,
                        message,
                    } => {
                        eprintln!(
                            "expect_shutdown reconnect #{attempt}: {} on {} ({message})",
                            node_names.get(&node).map(String::as_str).unwrap_or("?"),
                            machine_names
                                .get(&machine)
                                .map(String::as_str)
                                .unwrap_or("?")
                        );
                    }
                    SchedEvent::NodeFinished {
                        node,
                        machine,
                        status,
                        ..
                    } => {
                        eprintln!(
                            "expect_shutdown finished: {} on {}: {:?}",
                            node_names.get(&node).map(String::as_str).unwrap_or("?"),
                            machine_names
                                .get(&machine)
                                .map(String::as_str)
                                .unwrap_or("?"),
                            status
                        );
                    }
                    _ => {}
                }
            }
        });
        let (_cmd_tx, cmd_rx) = mpsc::channel(8);
        let report = tokio::time::timeout(
            tokio::time::Duration::from_secs(900),
            infra.apply(
                plan,
                Arc::new(AutoDenyInteractor),
                events,
                CancellationToken::new(),
                cmd_rx,
                factory,
                infrazeug_core::empty_native_executor(),
            ),
        )
        .await
        .map_err(|_| "expect_shutdown reboot apply timed out after 900s".to_string())?
        .map_err(|e| e.to_string())?;
        event_task.abort();
        let _ = tokio::fs::remove_dir_all(&short_run_root).await;
        if let Some(failed) = report
            .entries
            .iter()
            .find(|entry| entry.status == infrazeug_core::NodeStatus::Failed)
        {
            return Err(format!(
                "expect_shutdown reboot node failed: {} on {}: {}",
                failed.node_name,
                failed.machine_id,
                failed.message.clone().unwrap_or_default()
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgentUpdate {
    pub hostname: &'static str,
    pub changed: bool,
}

const AGENT_UPDATE_SCRIPT: &str = r#"
set -eu
export DEBIAN_FRONTEND=noninteractive
sudo -n apt-get -o DPkg::Lock::Timeout=180 update
plan="$(sudo -n apt-get -s -o Debug::NoLocking=1 dist-upgrade)"
printf '%s\n' "$plan"
if printf '%s\n' "$plan" | grep -Eq '^(Inst|Remv) '; then
  sudo -n apt-get -y \
    -o DPkg::Lock::Timeout=180 \
    -o Dpkg::Options::=--force-confdef \
    -o Dpkg::Options::=--force-confold \
    dist-upgrade
  printf '\ninfrazeug-agent-update: changed\n'
else
  printf '\ninfrazeug-agent-update: unchanged\n'
fi
"#;

const EXPECT_SHUTDOWN_REBOOT_SCRIPT: &str = r#"
set -eu
printf 'infrazeug-agent-update: rebooting\n'
sudo -n sh -c 'systemctl reboot >/dev/null 2>&1 || reboot >/dev/null 2>&1'
timeout 90 sh -c 'while :; do sleep 1; done'
"#;

fn port_offset() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}

async fn wait_lan_address(
    ssh: &SshConfig,
    identity: Option<&Path>,
    ip: &str,
    timeout_secs: u64,
) -> Result<(), String> {
    let script = format!("ip -4 addr | grep -q '{ip}/24' || ip -4 addr | grep -q '{ip}'");
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
    while tokio::time::Instant::now() < deadline {
        if ssh_run(ssh, identity, &["sh", "-c", &script]).await == Ok(0) {
            return Ok(());
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }
    Err(format!("address {ip} not configured"))
}

fn ssh_for_transport(ssh: &SshConfig, identity: Option<&Path>) -> Result<SshConfig, String> {
    let mut ssh = ssh.clone();
    ssh.extra_opts.push("StrictHostKeyChecking=no".into());
    ssh.extra_opts.push("UserKnownHostsFile=/dev/null".into());
    if let Some(identity) = identity {
        let path = identity.to_str().ok_or("identity path")?;
        ssh.extra_opts.push(format!("IdentityFile={path}"));
    }
    Ok(ssh)
}

pub fn resolve_debian_cloud_image() -> Result<PathBuf, String> {
    let path = std::env::var("INFRZEUG_DEBIAN_CLOUD_IMAGE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(
                std::env::var("HOME")
                    .map(|h| format!("{h}/.cache/infrazeug/debian-12-generic-amd64.qcow2"))
                    .unwrap_or_else(|_| "/var/lib/infrazeug/debian-12-generic-amd64.qcow2".into()),
            )
        });
    Ok(path)
}

pub(crate) fn load_or_generate_ssh(tmp: &Path) -> Result<(String, Option<PathBuf>), String> {
    if let Ok(pubkey) = std::env::var("INFRZEUG_QEMU_SSH_PUBKEY") {
        let identity = std::env::var("INFRZEUG_QEMU_SSH_KEY")
            .ok()
            .map(PathBuf::from);
        return Ok((pubkey, identity));
    }
    let key_path = tmp.join("id_ed25519");
    let status = std::process::Command::new("ssh-keygen")
        .args([
            "-t",
            "ed25519",
            "-N",
            "",
            "-f",
            key_path.to_str().ok_or("key path")?,
            "-q",
        ])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("ssh-keygen failed".into());
    }
    let pubkey = std::fs::read_to_string(format!("{}.pub", key_path.display()))
        .map_err(|e| e.to_string())?;
    Ok((pubkey, Some(key_path)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qemu_available;
    use tempfile::tempdir;

    fn vm_stack_test_enabled() -> bool {
        std::env::var("INFRZEUG_VM_STACK_TEST")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    /// Four Debian cloud VMs on a multicast LAN; SSH from the host.
    ///
    /// ```no_run
    /// INFRZEUG_VM_STACK_TEST=1 cargo test -p infrazeug-emulate-qemu debian_vm_stack_internal_network -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore = "requires Debian cloud qcow2 + qemu; run with INFRZEUG_VM_STACK_TEST=1"]
    async fn debian_vm_stack_internal_network() {
        if !vm_stack_test_enabled() {
            eprintln!("skip: set INFRZEUG_VM_STACK_TEST=1 to run this test");
            return;
        }
        if !qemu_available() {
            panic!("qemu-system-* not in PATH");
        }

        let image = resolve_debian_cloud_image().expect("image path");
        if !image.exists() {
            panic!(
                "download a Debian cloud image to {} or set INFRZEUG_DEBIAN_CLOUD_IMAGE\n  e.g. curl -Lo {} {}",
                image.display(),
                image.display(),
                DEBIAN_12_CLOUD_AMD64
            );
        }

        let tmp = tempdir().map_err(|e| e.to_string()).expect("tempdir");
        let (pubkey, identity) = load_or_generate_ssh(tmp.path()).expect("ssh key");
        let user = std::env::var("INFRZEUG_QEMU_SSH_USER").unwrap_or_else(|_| "debian".into());
        let host = QemuHost::new(tmp.path().to_path_buf());

        eprintln!("debian cloud image: {}", image.display());
        eprintln!("booting 4 microVMs (this may take several minutes on first boot)…");

        let stack = DebianVmStack::up(&host, &image, &pubkey, &user, identity)
            .await
            .expect("vm stack should start");

        eprintln!("guest LAN: {} ({})", stack.subnet(), stack.mcast());

        let result = async {
            stack.verify_debian().await?;
            stack.verify_internal_lan().await?;
            Ok::<(), String>(())
        }
        .await;

        stack.down().await.expect("vm stack teardown");

        result.expect("vm stack health checks");
    }

    /// Four Debian cloud VMs update their agents twice; the second pass must be a no-op.
    ///
    /// If the first update pass installs packages, only those changed guests are
    /// rebooted before the second pass. When no package updates are pending, the
    /// reboot phase is skipped.
    ///
    /// ```no_run
    /// INFRZEUG_VM_STACK_TEST=1 cargo test -p infrazeug-emulate-qemu debian_vm_stack_agent_updates_are_idempotent -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore = "requires Debian cloud qcow2 + qemu + networked apt; run with INFRZEUG_VM_STACK_TEST=1"]
    async fn debian_vm_stack_agent_updates_are_idempotent() {
        if !vm_stack_test_enabled() {
            eprintln!("skip: set INFRZEUG_VM_STACK_TEST=1 to run this test");
            return;
        }
        if !qemu_available() {
            panic!("qemu-system-* not in PATH");
        }

        let image = resolve_debian_cloud_image().expect("image path");
        if !image.exists() {
            panic!(
                "download a Debian cloud image to {} or set INFRZEUG_DEBIAN_CLOUD_IMAGE\n  e.g. curl -Lo {} {}",
                image.display(),
                image.display(),
                DEBIAN_12_CLOUD_AMD64
            );
        }

        let tmp = tempdir().map_err(|e| e.to_string()).expect("tempdir");
        let (pubkey, identity) = load_or_generate_ssh(tmp.path()).expect("ssh key");
        let user = std::env::var("INFRZEUG_QEMU_SSH_USER").unwrap_or_else(|_| "debian".into());
        let host = QemuHost::new(tmp.path().to_path_buf());

        eprintln!("debian cloud image: {}", image.display());
        eprintln!("booting 4 microVMs for agent update idempotence test...");

        let stack = DebianVmStack::up(&host, &image, &pubkey, &user, identity)
            .await
            .expect("vm stack should start");

        let result = async {
            let first = stack.update_agents().await?;
            let changed = first
                .iter()
                .filter(|u| u.changed)
                .map(|u| u.hostname)
                .collect::<Vec<_>>();
            if changed.is_empty() {
                eprintln!("no agent package updates pending; skipping reboot phase");
            } else {
                eprintln!("agent package updates changed: {}", changed.join(", "));
                stack.reboot_after_updates(&first).await?;
            }

            let second = stack.update_agents().await?;
            let still_changed = second
                .iter()
                .filter(|u| u.changed)
                .map(|u| u.hostname)
                .collect::<Vec<_>>();
            if !still_changed.is_empty() {
                return Err(format!(
                    "second agent update pass was not idempotent; changed again: {}",
                    still_changed.join(", ")
                ));
            }
            Ok::<(), String>(())
        }
        .await;

        stack.down().await.expect("vm stack teardown");

        result.expect("agent updates should be idempotent");
    }
}
