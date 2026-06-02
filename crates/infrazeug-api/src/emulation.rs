//! Container build + QEMU microVM setup for apply/test (M3 + M5).

use crate::Infra;
use infrazeug_core::id::RunId;
use infrazeug_core::machine::{MachineKind, OsFamily, OsHint};
use infrazeug_core::runtime::{RunGuard, RunMode};
use infrazeug_core::test_mode::{expand_machines, specs_from_machines};
use infrazeug_core::CoreError;
use infrazeug_core::TestReport;
use infrazeug_emulate::graph::BuildGraph;
use infrazeug_emulate::lock::{LockContext, LockFile};
use infrazeug_emulate::spec::{ContainerRef, EmulatedKind};
use infrazeug_emulate::EmulatedHost;
use infrazeug_emulate_oci::{
    build_graph, container_name, resolve_container_cli, PodmanExec, PodmanHost,
};
use infrazeug_emulate_qemu::{boot_microvm, stop_microvm, MicroVmHandle, QemuHost, SshGuestConfig};
use infrazeug_transport::TransportFactory;
use std::path::PathBuf;
use std::sync::Arc;

pub struct RunPrepare {
    pub infra: Infra,
    pub test_report: TestReport,
    pub lock: LockContext,
    microvm_handles: Vec<MicroVmHandle>,
}

pub fn infra_for_run(infra: &Infra, mode: RunMode, run_id: RunId) -> RunPrepare {
    let (effective, test_report) = expand_machines(&infra.machines, mode, run_id);
    let mut clone = infra.clone();
    clone
        .machines
        .retain(|m| !test_report.skipped.contains(&m.id));
    for m in &mut clone.machines {
        if let Some(em) = effective.iter().find(|e| e.id == m.id) {
            m.kind = em.kind.clone();
            m.lifecycle = em.lifecycle.clone();
        }
    }
    let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let lock = LockContext::open(&workspace, false).unwrap_or_else(|_| LockContext {
        path: workspace.join(LockFile::FILENAME),
        lock: LockFile::default(),
        unpinned: false,
    });
    RunPrepare {
        infra: clone,
        test_report,
        lock,
        microvm_handles: Vec::new(),
    }
}

fn ssh_guest_config() -> Result<SshGuestConfig, CoreError> {
    let pubkey = std::env::var("INFRZEUG_QEMU_SSH_PUBKEY").map_err(|_| {
        CoreError::other(
            "MicroVm requires INFRZEUG_QEMU_SSH_PUBKEY (ssh-ed25519 pubkey for cloud-init)",
        )
    })?;
    Ok(SshGuestConfig {
        user: std::env::var("INFRZEUG_QEMU_SSH_USER").unwrap_or_else(|_| "infrazeug".into()),
        ssh_pubkey: pubkey,
    })
}

async fn setup_microvms(prepared: &mut RunPrepare, guard: &RunGuard) -> Result<(), CoreError> {
    let needs_microvm = prepared.infra.machines.iter().any(|m| {
        m.like
            .as_ref()
            .is_some_and(|l| matches!(l.kind, EmulatedKind::MicroVm { .. }))
    });
    if !needs_microvm {
        return Ok(());
    }
    let guest = ssh_guest_config()?;
    let qemu_host = QemuHost::new(guard.path().to_path_buf());
    let run_uuid = guard.run_id.0;

    for machine in &mut prepared.infra.machines {
        let Some(like) = &machine.like else {
            continue;
        };
        let EmulatedKind::MicroVm { image, qemu } = &like.kind else {
            continue;
        };
        let boot = boot_microvm(&qemu_host, run_uuid, &machine.name, image, qemu, &guest)
            .await
            .map_err(CoreError::other)?;
        prepared.microvm_handles.push(boot.handle.clone());
        machine.kind = MachineKind::Remote {
            ssh: boot.ssh,
            os: Some(OsHint {
                family: OsFamily::Linux,
                distro: Some("qemu-microvm".into()),
                version: None,
                arch: None,
            }),
        };
    }
    Ok(())
}

pub async fn setup_emulation(
    prepared: &mut RunPrepare,
    guard: &RunGuard,
    factory: &Arc<TransportFactory>,
) -> Result<(), CoreError> {
    setup_microvms(prepared, guard).await?;

    let specs = specs_from_machines(&prepared.infra.machines);
    if specs.is_empty() {
        return Ok(());
    }

    let graph = BuildGraph::from_specs(specs).map_err(|e| CoreError::other(e.to_string()))?;
    let workspace = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut lock = prepared.lock.clone();
    lock.lock
        .refresh_from_graph(&graph.specs, &Default::default())
        .map_err(|e| CoreError::other(e.to_string()))?;
    let _ = lock.save();

    let oci = resolve_container_cli().await.ok_or_else(|| {
        CoreError::other(
            "no OCI runtime (install podman or docker, or set INFRZEUG_CONTAINER_RUNTIME)",
        )
    })?;
    let host = PodmanHost::with_cli(workspace, oci.clone());
    let run_uuid = guard.run_id.0;
    let built = build_graph(&host, run_uuid, &graph)
        .await
        .map_err(|e| CoreError::other(e.to_string()))?;

    for machine in &prepared.infra.machines {
        let MachineKind::Container(container_ref) = &machine.kind else {
            continue;
        };
        let image = match container_ref {
            ContainerRef::Prebuilt(img) => img.reference(),
            ContainerRef::Spec(spec) => {
                let id = spec.id();
                built
                    .get(&id)
                    .map(|b| b.image_ref.clone())
                    .ok_or_else(|| CoreError::other(format!("missing build for {}", id.0)))?
            }
        };
        let cname = container_name(run_uuid, &machine.name);
        let running = host
            .run_container(run_uuid, &image, &cname)
            .await
            .map_err(|e| CoreError::other(e.to_string()))?;
        let exec = PodmanExec {
            runtime: oci.bin.clone(),
            container: running.name,
        };
        factory.register_container(machine.id, exec).await;
    }
    Ok(())
}

pub async fn teardown_containers(prepared: &RunPrepare, guard: &RunGuard) -> Result<(), CoreError> {
    for handle in &prepared.microvm_handles {
        let _ = stop_microvm(handle).await;
    }
    let host = PodmanHost::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let run_uuid = guard.run_id.0;
    for machine in &prepared.infra.machines {
        if matches!(machine.kind, MachineKind::Container(_)) {
            let cname = container_name(run_uuid, &machine.name);
            let _ = host.stop_container(&cname).await;
        }
    }
    Ok(())
}
