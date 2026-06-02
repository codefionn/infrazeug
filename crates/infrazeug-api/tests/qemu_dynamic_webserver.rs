//! End-to-end (QEMU): spawn microVMs, apply a **dynamic machine group** playbook
//! that brings up a webserver on each discovered VM, then apply a **second,
//! traditional (static-machine)** playbook that validates every machine is serving.
//!
//! Using a static playbook for validation is deliberate: it checks the result via
//! a code path independent of the dynamic-group machinery, so a bug in discovery /
//! fan-out can't mask itself by failing identically in both phases.
//!
//! Ignored by default and additionally gated on `INFRZEUG_VM_STACK_TEST=1` (the
//! convention `scripts/run-infra-tests.sh` uses). Needs `qemu-system-*` and a
//! Debian cloud image (`INFRZEUG_DEBIAN_CLOUD_IMAGE`, or the cached default). An
//! SSH keypair is generated on the fly unless `INFRZEUG_QEMU_SSH_PUBKEY` /
//! `INFRZEUG_QEMU_SSH_KEY` are set.
//!
//! ```sh
//! INFRZEUG_VM_STACK_TEST=1 \
//! INFRZEUG_DEBIAN_CLOUD_IMAGE=~/.cache/infrazeug/debian-12-generic-amd64.qcow2 \
//! cargo test -p infrazeug-api --test qemu_dynamic_webserver -- --ignored --nocapture
//! ```

use async_trait::async_trait;
use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{apply_bundle, ApplyOptions, PlaybookBundle};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::machine::{DiscoveredMachine, SshConfig};
use infrazeug_core::node::NodeStatus;
use infrazeug_core::RuntimeConfig;
use infrazeug_emulate::spec::{QemuConfig, VmImage};
use infrazeug_emulate_qemu::{
    boot_microvm, create_overlay, qemu_available, stop_microvm, QemuHost, SshGuestConfig,
};
use infrazeug_native::{
    NativeError, NativeResult, NodeCtx, NodeMethod, Result as NativeMethodResult,
};
use infrazeug_shell::ShellOp;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use uuid::Uuid;

const LABEL: &str = "webfarm";
const HOSTS: [&str; 2] = ["web-1", "web-2"];
const MARKER: &str = "infrazeug-ok";

// Stable ids. Setup is one dynamic group; validation is per-VM static machines.
const CONTROLLER: u128 = 0x01;
const SETUP_DISC: u128 = 0x10;
const SETUP_CONNECT: u128 = 0x11;
const SETUP_SERVE: u128 = 0x12;
const VAL_MACHINE_BASE: u128 = 0x200;
const VAL_CHECK_BASE: u128 = 0x300;

fn mid(u: u128) -> MachineId {
    MachineId(Uuid::from_u128(u))
}
fn nid(u: u128) -> NodeId {
    NodeId(Uuid::from_u128(u))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Unit {}

/// Discovery method returning a fixed machine list (the VMs this test booted).
#[derive(Clone)]
struct StaticDiscovery {
    machines: Vec<DiscoveredMachine>,
}

#[async_trait]
impl NodeMethod for StaticDiscovery {
    type Input = Unit;
    type Output = Vec<DiscoveredMachine>;

    fn name(&self) -> &'static str {
        "test.qemu.static_discovery"
    }

    async fn execute(&self, _ctx: &NodeCtx, _input: Unit) -> NativeMethodResult<NativeResult> {
        NativeResult::changed(format!("discovered {} machine(s)", self.machines.len()))
            .with_json_capture(&self.machines)
            .map_err(|e| NativeError::other(e.to_string()))
    }
}

fn runtime(name: &str) -> RuntimeConfig {
    RuntimeConfig {
        run_root: std::env::temp_dir().join(format!("infrazeug-qemu-dyn-{name}")),
        vault_store: None,
    }
}

/// Shell op that fetches the local page and exits non-zero unless it has the marker.
fn check_op() -> ShellOp {
    ShellOp::run(vec![
        "sh".into(),
        "-c".into(),
        format!(
            "python3 -c 'import urllib.request,sys; \
             sys.exit(0 if b\"{MARKER}\" in urllib.request.urlopen(\"http://127.0.0.1:8080/\").read() else 1)'"
        ),
    ])
}

/// Setup playbook: a **dynamic machine group** that discovers the VMs and, on each,
/// writes an index page and starts a detached python webserver on :8080.
fn build_setup(machines: &[DiscoveredMachine]) -> anyhow::Result<PlaybookBundle> {
    let controller = mid(CONTROLLER);
    let serve = ShellOp::run(vec![
        "sh".into(),
        "-c".into(),
        format!(
            "mkdir -p /tmp/webroot && printf '{MARKER}\\n' > /tmp/webroot/index.html && \
             (setsid python3 -m http.server 8080 --directory /tmp/webroot \
              >/tmp/web.log 2>&1 </dev/null &) && sleep 1"
        ),
    ]);

    Ok(InfraBuilder::new()
        .machine(builder::controller(controller))?
        .discover_machines(
            nid(SETUP_DISC),
            "discover",
            controller,
            LABEL,
            StaticDiscovery {
                machines: machines.to_vec(),
            },
            Unit {},
        )?
        .for_each_machine(move |m| {
            m.connectivity(nid(SETUP_CONNECT), "connect");
            m.shell(nid(SETUP_SERVE), "serve", serve, [nid(SETUP_CONNECT)]);
        })?
        .build()
        .with_runtime(runtime("setup")))
}

/// Validation playbook: a **traditional static-machine** playbook — one remote
/// machine per VM with a check node — independent of the dynamic-group mechanism.
/// Returns the per-VM `(name, check node id)` so the test can assert each one.
fn build_validate_static(
    machines: &[DiscoveredMachine],
) -> anyhow::Result<(PlaybookBundle, Vec<(String, NodeId)>)> {
    let mut b = InfraBuilder::new();
    let mut checks = Vec::new();
    for (i, m) in machines.iter().enumerate() {
        let machine_id = mid(VAL_MACHINE_BASE + i as u128);
        let check_id = nid(VAL_CHECK_BASE + i as u128);
        b = b
            .machine(builder::remote(machine_id, &m.name, m.ssh.clone()))?
            .shell_on_machine(
                check_id,
                &format!("check-{}", m.name),
                machine_id,
                check_op(),
            )?;
        checks.push((m.name.clone(), check_id));
    }
    Ok((b.build().with_runtime(runtime("validate")), checks))
}

fn vm_stack_test_enabled() -> bool {
    std::env::var("INFRZEUG_VM_STACK_TEST")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn resolve_image() -> PathBuf {
    std::env::var("INFRZEUG_DEBIAN_CLOUD_IMAGE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(
                std::env::var("HOME")
                    .map(|h| format!("{h}/.cache/infrazeug/debian-12-generic-amd64.qcow2"))
                    .unwrap_or_else(|_| "/var/lib/infrazeug/debian-12-generic-amd64.qcow2".into()),
            )
        })
}

/// Use a provided keypair or generate an ephemeral one (mirrors the vm-stack tests).
fn load_or_generate_ssh(dir: &Path) -> anyhow::Result<(String, Option<PathBuf>)> {
    if let Ok(pubkey) = std::env::var("INFRZEUG_QEMU_SSH_PUBKEY") {
        let identity = std::env::var("INFRZEUG_QEMU_SSH_KEY")
            .ok()
            .map(PathBuf::from);
        return Ok((pubkey, identity));
    }
    std::fs::create_dir_all(dir)?;
    let key_path = dir.join("id_ed25519");
    let status = std::process::Command::new("ssh-keygen")
        .args([
            "-t",
            "ed25519",
            "-N",
            "",
            "-f",
            key_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("key path"))?,
            "-q",
        ])
        .status()?;
    anyhow::ensure!(status.success(), "ssh-keygen failed");
    let pubkey = std::fs::read_to_string(format!("{}.pub", key_path.display()))?;
    Ok((pubkey, Some(key_path)))
}

/// Poll real SSH login until it succeeds — `boot_microvm` only waits for qemu's
/// forwarded port to open, not for cloud-init to finish injecting the key + sshd.
async fn wait_ssh_ready(port: u16, user: &str, identity: Option<&Path>, secs: u64) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        let mut cmd = tokio::process::Command::new("ssh");
        cmd.args([
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "UserKnownHostsFile=/dev/null",
            "-o",
            "IdentitiesOnly=yes",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=5",
        ]);
        if let Some(id) = identity {
            cmd.arg("-i").arg(id);
        }
        cmd.arg("-p")
            .arg(port.to_string())
            .arg(format!("{user}@127.0.0.1"))
            .arg("true");
        if let Ok(status) = cmd.status().await {
            if status.success() {
                return true;
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

/// SSH options the push agent needs for an ephemeral guest (identity + relaxed
/// host-key checking).
fn ssh_for_agent(base: &SshConfig, identity: Option<&Path>) -> SshConfig {
    let mut ssh = base.clone();
    ssh.extra_opts = vec![
        "StrictHostKeyChecking=no".to_string(),
        "UserKnownHostsFile=/dev/null".to_string(),
        "IdentitiesOnly=yes".to_string(),
    ];
    if let Some(id) = identity {
        ssh.extra_opts
            .push(format!("IdentityFile={}", id.display()));
    }
    ssh
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "spawns QEMU VMs; run with INFRZEUG_VM_STACK_TEST=1 + a Debian cloud image"]
async fn dynamic_group_sets_up_static_playbook_validates() -> anyhow::Result<()> {
    if !vm_stack_test_enabled() {
        eprintln!("skip: set INFRZEUG_VM_STACK_TEST=1 to run this test");
        return Ok(());
    }
    if !qemu_available() {
        eprintln!("skip: qemu-system-* not found in PATH");
        return Ok(());
    }
    let image_path = resolve_image();
    if !image_path.exists() {
        eprintln!(
            "skip: Debian cloud image missing at {} (set INFRZEUG_DEBIAN_CLOUD_IMAGE)",
            image_path.display()
        );
        return Ok(());
    }

    let run_id = Uuid::new_v4();
    let key_dir = std::env::temp_dir().join(format!("iz-qemu-dyn-keys-{run_id}"));
    let (pubkey, identity) = load_or_generate_ssh(&key_dir)?;
    let user = std::env::var("INFRZEUG_QEMU_SSH_USER").unwrap_or_else(|_| "debian".into());

    let host = QemuHost::new(std::env::temp_dir().join("infrazeug-qemu-dyn-test"));
    let qemu = QemuConfig {
        memory_mb: std::env::var("INFRZEUG_VM_STACK_MEM_MB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1024),
    };
    let guest = SshGuestConfig {
        user: user.clone(),
        ssh_pubkey: pubkey,
    };

    // Boot the VMs, then apply setup (dynamic group) and validate (static). Run it
    // all inside one block so the teardown below always stops every VM that booted —
    // even if a later boot or an apply fails.
    let mut boots: Vec<(&str, _)> = Vec::new();
    let result: anyhow::Result<_> = async {
        for name in HOSTS {
            // Each VM gets its own copy-on-write overlay over the shared read-only
            // base image; booting qemu against the base directly write-locks it, so
            // a second VM from the same base would otherwise fail to start.
            let overlay_dir = host
                .run_workspace
                .join(run_id.to_string())
                .join("overlays")
                .join(name);
            std::fs::create_dir_all(&overlay_dir)?;
            let overlay = overlay_dir.join("disk.qcow2");
            create_overlay(&image_path, &overlay)
                .await
                .map_err(|e| anyhow::anyhow!("overlay {name}: {e}"))?;
            let vm_image = VmImage::RemoteQcow2(overlay.display().to_string());
            let boot = boot_microvm(&host, run_id, name, &vm_image, &qemu, &guest)
                .await
                .map_err(|e| anyhow::anyhow!("boot {name}: {e}"))?;
            // Track it before waiting so the teardown below stops it even if the
            // readiness wait fails.
            let port = boot.handle.ssh_port;
            boots.push((name, boot));
            if !wait_ssh_ready(port, &user, identity.as_deref(), 240).await {
                anyhow::bail!("{name}: SSH/cloud-init not ready on port {port}");
            }
        }

        let machines: Vec<DiscoveredMachine> = boots
            .iter()
            .map(|(name, boot)| DiscoveredMachine {
                name: (*name).to_string(),
                ssh: ssh_for_agent(&boot.ssh, identity.as_deref()),
                vars: Default::default(),
                tags: Vec::new(),
                os: None,
            })
            .collect();

        let setup = apply_bundle(&build_setup(&machines)?, ApplyOptions::default()).await?;
        let (validate_bundle, checks) = build_validate_static(&machines)?;
        let validate = apply_bundle(&validate_bundle, ApplyOptions::default()).await?;
        anyhow::Ok((setup, validate, checks))
    }
    .await;

    for (_, boot) in &boots {
        let _ = stop_microvm(&boot.handle).await;
    }
    let _ = std::fs::remove_dir_all(&key_dir);
    let (setup, validate, checks) = result?;

    // Setup (dynamic fan-out): nothing failed on any machine.
    let setup_failures: Vec<_> = setup
        .entries
        .iter()
        .filter(|e| e.status == NodeStatus::Failed)
        .map(|e| format!("{}@{}", e.node_name, e.machine_id))
        .collect();
    assert!(
        setup_failures.is_empty(),
        "setup failures: {setup_failures:?}"
    );

    // Validation (static playbook): every machine's independent check succeeded.
    for (name, check_id) in &checks {
        let entry = validate
            .entries
            .iter()
            .find(|e| e.node_id == *check_id)
            .unwrap_or_else(|| panic!("no check entry for {name}"));
        assert!(
            matches!(entry.status, NodeStatus::Changed | NodeStatus::Unchanged),
            "machine {name} did not validate: {:?}",
            entry.status
        );
    }

    Ok(())
}
