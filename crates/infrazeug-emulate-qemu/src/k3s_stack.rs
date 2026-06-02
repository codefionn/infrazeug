//! Single-node k3s + Helm lab on a Debian QEMU microVM (integration test only).
//!
//! Stack: Cilium CNI, OpenEBS hostpath, CloudNativePG operator, Open WebUI on CNPG.

use crate::cloud_init::{CloudInitSeed, SshGuestConfig};
use crate::guest_ssh::{ssh_capture, ssh_run, ssh_upload, wait_cloud_init};
use crate::host::{stop_microvm, MicroVmHandle, QemuHost};
use crate::overlay::create_overlay_sized;
use crate::spawn::{qemu_available, spawn_qemu, wait_ssh_port, QemuArch, QemuSpawn};
use crate::vm_stack::resolve_debian_cloud_image;
use infrazeug_core::machine::SshConfig;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const STACK_LABEL: &str = "infrazeug.k3s_helm_stack";
const HOSTNAME: &str = "iz-k3s";

/// Installs k3s (no flannel/kube-proxy), Cilium, OpenEBS hostpath, CNPG, and Open WebUI.
const BOOTSTRAP_SH: &str = r#"#!/usr/bin/env bash
set -euo pipefail
export KUBECONFIG=/etc/rancher/k3s/k3s.yaml
LOG=/tmp/iz-k3s-bootstrap.log
: >"$LOG"
chmod 666 "$LOG"
exec >>"$LOG" 2>&1

log() { echo "[k3s-stack] $(date '+%Y-%m-%dT%H:%M:%S') $*"; }

log "bootstrap starting"

helm_repo_add() {
  if helm repo list 2>/dev/null | awk 'NR>1 {print $1}' | grep -qx "$1"; then
    return 0
  fi
  helm repo add "$1" "$2"
}

# k3s often reports 127.0.0.1 as InternalIP; Cilium needs the routable host address.
# The node can carry both an IPv4 and an IPv6 InternalIP (QEMU SLAAC), and
# hostname -I lists both too. Cilium's k8sServiceHost needs a single IPv4.
first_ipv4() { tr ' ' '\n' | grep -E '^[0-9]+(\.[0-9]+){3}$' | head -n1; }
node_ip() {
  local ip
  ip=$(k3s kubectl get nodes -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}' 2>/dev/null | first_ipv4 || true)
  if [[ -z "$ip" || "$ip" == "127.0.0.1" ]]; then
    ip=$(hostname -I | first_ipv4)
  fi
  if [[ -z "$ip" ]]; then
    log "fatal: could not determine node IP for Cilium"
    exit 1
  fi
  echo "$ip"
}

wait_for_network() {
  for _ in $(seq 1 60); do
    if curl -sfI --connect-timeout 5 https://get.k3s.io >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  log "fatal: no outbound network"
  exit 1
}

wait_for_apt() {
  for _ in $(seq 1 120); do
    if ! fuser \
      /var/lib/dpkg/lock-frontend \
      /var/lib/dpkg/lock \
      /var/lib/apt/lists/lock \
      /var/cache/apt/archives/lock >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  log "warning: apt lock still held; continuing"
}

retry_step() {
  local label="$1"
  shift
  local status=0
  for attempt in $(seq 1 5); do
    log "${label} (attempt ${attempt}/5)"
    if "$@"; then
      return 0
    else
      status=$?
    fi
    log "warning: ${label} failed with exit ${status}"
    sleep $((attempt * 5))
  done
  log "fatal: ${label} failed after retries"
  return "$status"
}

ensure_packages() {
  wait_for_apt
  if command -v curl >/dev/null && command -v gpg >/dev/null; then
    wait_for_network
    return 0
  fi
  log "installing curl and prerequisites via apt"
  export DEBIAN_FRONTEND=noninteractive
  retry_step "apt-get update" \
    apt-get -o DPkg::Lock::Timeout=180 -o Acquire::Retries=3 update
  retry_step "apt-get install prerequisites" \
    apt-get -o DPkg::Lock::Timeout=180 -o Acquire::Retries=3 install -y --no-install-recommends \
      curl ca-certificates gnupg apt-transport-https
  wait_for_network
}

ensure_packages

PRE_BOOT_IP="$(hostname -I | awk '{print $1}')"
if [[ -z "$PRE_BOOT_IP" ]]; then
  log "fatal: could not determine host IP before k3s install"
  exit 1
fi

log "install k3s (tls-san=${PRE_BOOT_IP})"
curl -sfL https://get.k3s.io | INSTALL_K3S_EXEC="server \
  --write-kubeconfig-mode 644 \
  --flannel-backend=none \
  --disable-network-policy \
  --cluster-cidr=10.42.0.0/16 \
  --disable-kube-proxy \
  --disable traefik \
  --disable servicelb \
  --tls-san ${PRE_BOOT_IP}" sh -

# With flannel-backend=none and no CNI yet, the node stays NotReady until Cilium
# is installed below — so only wait for it to register here, not to become Ready.
log "wait for node registration"
for _ in $(seq 1 120); do
  [ "$(k3s kubectl get nodes --no-headers 2>/dev/null | wc -l)" -ge 1 ] && break
  sleep 5
done
k3s kubectl get nodes

log "install helm"
curl -fsSL https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash

log "helm repos"
helm_repo_add cilium https://helm.cilium.io/
helm_repo_add openebs https://openebs.github.io/openebs
helm_repo_add cnpg https://cloudnative-pg.github.io/charts
helm_repo_add open-webui https://helm.openwebui.com/ || helm_repo_add open-webui https://open-webui.github.io/helm-charts
helm repo update

NODE_IP="$(node_ip)"
log "node IP for Cilium k8sServiceHost: ${NODE_IP}"

log "install cilium (tunnel mode — QEMU user netdev cannot do native routing)"
helm upgrade --install cilium cilium/cilium -n kube-system --wait --timeout 25m \
  --set operator.replicas=1 \
  --set kubeProxyReplacement=true \
  --set k8sServiceHost="${NODE_IP}" \
  --set k8sServicePort=6443 \
  --set ipam.operator.clusterPoolIPv4PodCIDRList="{10.42.0.0/16}" \
  --set socketLB.enabled=true

# helm --wait races on a freshly-created DaemonSet (sees 0 desired and returns
# before the agent is even scheduled), so gate on the rollout explicitly.
cilium_diagnostics() {
  log "cilium diagnostics follow"
  k3s kubectl -n kube-system get pods -o wide || true
  k3s kubectl -n kube-system describe ds/cilium || true
  local pod
  pod=$(k3s kubectl -n kube-system get pods -l k8s-app=cilium -o name 2>/dev/null | head -n1)
  if [[ -n "$pod" ]]; then
    k3s kubectl -n kube-system describe "$pod" || true
    k3s kubectl -n kube-system logs "$pod" --all-containers --tail=150 --prefix || true
  fi
}

log "wait for cilium rollout"
k3s kubectl -n kube-system rollout status deploy/cilium-operator --timeout=600s || { cilium_diagnostics; exit 1; }
k3s kubectl -n kube-system rollout status ds/cilium --timeout=600s || { cilium_diagnostics; exit 1; }

log "wait for node Ready"
if ! k3s kubectl wait --for=condition=Ready node --all --timeout=300s; then
  log "node not Ready"
  k3s kubectl get nodes -o wide || true
  k3s kubectl describe nodes || true
  cilium_diagnostics
  exit 1
fi

log "install openebs (local hostpath only)"
helm upgrade --install openebs openebs/openebs -n openebs --create-namespace --wait --timeout 20m \
  --set engines.local.lvm.enabled=false \
  --set engines.local.zfs.enabled=false \
  --set engines.local.rawfile.enabled=false \
  --set engines.replicated.mayastor.enabled=false \
  --set loki.enabled=false \
  --set alloy.enabled=false

for _ in $(seq 1 90); do
  k3s kubectl get storageclass openebs-hostpath >/dev/null 2>&1 && break
  sleep 5
done
k3s kubectl get storageclass openebs-hostpath

log "install cnpg operator"
helm upgrade --install cnpg cnpg/cloudnative-pg -n cnpg-system --create-namespace --wait --timeout 20m

log "cnpg cluster for openwebui"
k3s kubectl create namespace apps --dry-run=client -o yaml | k3s kubectl apply -f -
k3s kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Secret
metadata:
  name: openwebui-db-credentials
  namespace: apps
type: kubernetes.io/basic-auth
stringData:
  username: openwebui
  password: iz-test-k3s-secret
---
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: openwebui-db
  namespace: apps
spec:
  instances: 1
  storage:
    size: 2Gi
    storageClass: openebs-hostpath
  bootstrap:
    initdb:
      database: openwebui
      owner: openwebui
      secret:
        name: openwebui-db-credentials
EOF

k3s kubectl wait --for=condition=Ready cluster/openwebui-db -n apps --timeout=1200s

log "install open-webui"
DB_HOST="openwebui-db-rw.apps.svc.cluster.local"
# Disable the heavyweight optional subcharts: this lab only exercises Open WebUI
# on the external CNPG database, and bundled Ollama/pipelines/tika pull multi-GB
# images that never settle inside the QEMU guest.
if ! helm upgrade --install open-webui open-webui/open-webui -n apps --wait --timeout 25m \
  --set ollama.enabled=false \
  --set pipelines.enabled=false \
  --set tika.enabled=false \
  --set websocket.enabled=false \
  --set postgresql.enabled=false \
  --set databaseUrl="postgresql://openwebui:iz-test-k3s-secret@${DB_HOST}:5432/openwebui"; then
  log "open-webui install failed — diagnostics follow"
  k3s kubectl -n apps get pods -o wide || true
  k3s kubectl -n apps describe pods -l app.kubernetes.io/instance=open-webui || true
  k3s kubectl -n apps logs -l app.kubernetes.io/instance=open-webui --all-containers --tail=120 --prefix || true
  exit 1
fi

log "bootstrap complete"
"#;

const BOOTSTRAP_LOG: &str = "/tmp/iz-k3s-bootstrap.log";
const BOOTSTRAP_EXIT: &str = "/tmp/iz-k3s-bootstrap.exit";
const BOOTSTRAP_RUNNER_LOG: &str = "/tmp/iz-k3s-bootstrap.runner.log";

async fn bootstrap_failure_message(
    ssh: &SshConfig,
    identity: Option<&Path>,
    exit_code: i32,
    ssh_output: &str,
) -> String {
    let (_, log_tail) = ssh_capture(
        ssh,
        identity,
        &format!(
            "echo '=== {BOOTSTRAP_LOG} ==='; \
             tail -n 100 {BOOTSTRAP_LOG} 2>/dev/null || echo '(no bootstrap log)'; \
             echo '=== {BOOTSTRAP_RUNNER_LOG} ==='; \
             tail -n 100 {BOOTSTRAP_RUNNER_LOG} 2>/dev/null || echo '(no runner log)'"
        ),
    )
    .await
    .unwrap_or((-1, String::new()));
    format!(
        "k3s helm bootstrap failed (exit {exit_code})\n\
         --- bootstrap diagnostics ---\n{log_tail}\n\
         --- ssh stdout/stderr ---\n{ssh_output}"
    )
}

async fn run_bootstrap(ssh: &SshConfig, identity: Option<&Path>) -> Result<(), String> {
    ssh_upload(
        ssh,
        identity,
        "/tmp/iz-k3s-bootstrap.sh",
        BOOTSTRAP_SH.as_bytes(),
        "755",
    )
    .await?;

    let start = format!(
        "sudo -n rm -f {BOOTSTRAP_EXIT} {BOOTSTRAP_RUNNER_LOG}; \
         sudo -n sh -c 'nohup bash -c '\\''/tmp/iz-k3s-bootstrap.sh; code=$?; \
         printf \"%s\\n\" \"$code\" > {BOOTSTRAP_EXIT}'\\'' \
         > {BOOTSTRAP_RUNNER_LOG} 2>&1 < /dev/null &'"
    );
    let (code, output) = ssh_capture(ssh, identity, &start).await?;
    if code != 0 {
        return Err(format!(
            "failed to start k3s bootstrap (exit {code}):\n{output}"
        ));
    }

    let timeout_secs = std::env::var("INFRZEUG_K3S_STACK_BOOTSTRAP_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3600);
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
    eprintln!("k3s bootstrap started; polling {BOOTSTRAP_LOG} for up to {timeout_secs}s");

    let mut last_log_lines = 0usize;
    while tokio::time::Instant::now() < deadline {
        let (_, new_log) = ssh_capture(
            ssh,
            identity,
            &format!(
                "if [ -f {BOOTSTRAP_LOG} ]; then \
                   total=$(wc -l < {BOOTSTRAP_LOG}); \
                   start=$(({} + 1)); \
                   if [ \"$total\" -ge \"$start\" ]; then tail -n +$start {BOOTSTRAP_LOG}; fi; \
                   echo __infrazeug_lines__$total; \
                 fi",
                last_log_lines
            ),
        )
        .await
        .unwrap_or((-1, String::new()));
        if let Some((log, lines)) = split_log_poll(&new_log) {
            if !log.trim().is_empty() {
                eprint!("{log}");
                if !log.ends_with('\n') {
                    eprintln!();
                }
            }
            last_log_lines = lines;
        }

        let (exit_probe, exit_output) = ssh_capture(
            ssh,
            identity,
            &format!("if [ -f {BOOTSTRAP_EXIT} ]; then echo __infrazeug_exit__; cat {BOOTSTRAP_EXIT}; fi"),
        )
        .await
        .unwrap_or((-1, String::new()));
        if exit_probe == 0 {
            if let Some(pos) = exit_output.rfind("__infrazeug_exit__") {
                let exit_text = exit_output[pos + "__infrazeug_exit__".len()..]
                    .lines()
                    .find_map(|line| {
                        let trimmed = line.trim();
                        (!trimmed.is_empty()).then_some(trimmed)
                    });
                let exit_code = exit_text
                    .and_then(|text| text.parse::<i32>().ok())
                    .unwrap_or(1);
                if exit_code == 0 {
                    return Ok(());
                }
                return Err(bootstrap_failure_message(ssh, identity, exit_code, "").await);
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    }

    let _ = ssh_capture(
        ssh,
        identity,
        "sudo pkill -f /tmp/iz-k3s-bootstrap.sh || true",
    )
    .await;
    Err(bootstrap_failure_message(ssh, identity, -1, "bootstrap timeout").await)
}

fn split_log_poll(output: &str) -> Option<(&str, usize)> {
    let marker = "__infrazeug_lines__";
    let pos = output.rfind(marker)?;
    let (log, tail) = output.split_at(pos);
    let line_count = tail.trim_start_matches(marker).lines().find_map(|line| {
        let trimmed = line.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    })?;
    let lines = line_count.parse::<usize>().ok()?;
    Some((log, lines))
}

/// k3s single-node cluster with Cilium, OpenEBS, CNPG, and Open WebUI.
#[allow(dead_code)]
pub struct K3sHelmStack {
    workspace: PathBuf,
    ssh: SshConfig,
    ssh_identity: Option<PathBuf>,
    handle: MicroVmHandle,
}

impl K3sHelmStack {
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
        if !base_image.exists() {
            return Err(format!(
                "debian cloud image not found: {} (set INFRZEUG_DEBIAN_CLOUD_IMAGE)",
                base_image.display()
            ));
        }

        let run_id = Uuid::new_v4().to_string();
        let workspace = host.run_workspace.join(&run_id).join("k3s-stack");
        let short = &run_id[..8];
        tokio::fs::create_dir_all(&workspace)
            .await
            .map_err(|e| e.to_string())?;

        let vm_ws = workspace.join(HOSTNAME);
        tokio::fs::create_dir_all(&vm_ws)
            .await
            .map_err(|e| e.to_string())?;
        let overlay = vm_ws.join("disk.qcow2");
        let disk_gb = std::env::var("INFRZEUG_K3S_STACK_DISK_GB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);
        create_overlay_sized(base_image, &overlay, Some(disk_gb)).await?;

        let memory_mb = std::env::var("INFRZEUG_K3S_STACK_MEM_MB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8192);
        let ssh_port = 4500 + (port_offset() % 400) as u16;

        let guest_ssh = SshGuestConfig {
            user: ssh_user.into(),
            ssh_pubkey: ssh_pubkey.into(),
        };
        let seed = CloudInitSeed {
            workspace: vm_ws.clone(),
        };
        let seed_iso = seed.write_lab_guest(&guest_ssh, HOSTNAME).await?;

        let arch = QemuArch::detect();
        let kvm = Path::new("/dev/kvm").exists();
        let QemuSpawn {
            mut child,
            ssh_port: actual_port,
        } = spawn_qemu(
            arch,
            &overlay,
            &seed_iso,
            memory_mb.max(6144),
            ssh_port,
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

        let ssh = SshConfig::new(format!("127.0.0.1:{actual_port}")).with_user(ssh_user);

        if wait_ssh_port(actual_port, 240).await.is_err() {
            let _ = child.kill().await;
            return Err(format!("SSH not ready on {HOSTNAME} (port {actual_port})"));
        }
        if let Err(e) = wait_cloud_init(&ssh, ssh_identity.as_deref(), 420).await {
            let _ = child.kill().await;
            return Err(format!("{HOSTNAME}: {e}"));
        }

        if let Err(e) = run_bootstrap(&ssh, ssh_identity.as_deref()).await {
            let _ = child.kill().await;
            return Err(e);
        }

        tokio::spawn(async move {
            let _ = child.wait().await;
        });
        let handle = MicroVmHandle {
            name: format!("{HOSTNAME}-{short}"),
            ssh_port: actual_port,
            pid_file: vm_ws.join("qemu.pid"),
        };

        Ok(Self {
            workspace,
            ssh,
            ssh_identity,
            handle,
        })
    }

    /// Cilium agent, OpenEBS hostpath SC, CNPG operator, CNPG cluster, and Open WebUI are healthy.
    pub async fn verify(&self) -> Result<(), String> {
        let kube = "export KUBECONFIG=/etc/rancher/k3s/k3s.yaml; ";

        let cilium = format!(
            "{kube}k3s kubectl -n kube-system get pods -l app.kubernetes.io/name=cilium-agent -o jsonpath='{{.items[*].status.phase}}' | grep -q Running"
        );
        if ssh_run(
            &self.ssh,
            self.ssh_identity.as_deref(),
            &["sh", "-c", &cilium],
        )
        .await?
            != 0
        {
            return Err("cilium agent not Running".into());
        }

        let sc = format!("{kube}k3s kubectl get storageclass openebs-hostpath");
        if ssh_run(&self.ssh, self.ssh_identity.as_deref(), &["sh", "-c", &sc]).await? != 0 {
            return Err("openebs-hostpath StorageClass missing".into());
        }

        let cnpg = format!(
            "{kube}k3s kubectl -n cnpg-system wait --for=condition=Available deployment -l app.kubernetes.io/name=cloudnative-pg --timeout=120s"
        );
        if ssh_run(
            &self.ssh,
            self.ssh_identity.as_deref(),
            &["sh", "-c", &cnpg],
        )
        .await?
            != 0
        {
            return Err("cnpg operator not Available".into());
        }

        let cluster = format!(
            "{kube}k3s kubectl -n apps wait --for=condition=Ready cluster/openwebui-db --timeout=120s"
        );
        if ssh_run(
            &self.ssh,
            self.ssh_identity.as_deref(),
            &["sh", "-c", &cluster],
        )
        .await?
            != 0
        {
            return Err("cnpg openwebui-db cluster not Ready".into());
        }

        // Open WebUI is deployed as a StatefulSet, so wait on pod readiness by
        // instance label rather than assuming a Deployment.
        let webui = format!(
            "{kube}k3s kubectl -n apps wait --for=condition=Ready pod -l app.kubernetes.io/instance=open-webui --timeout=300s"
        );
        if ssh_run(
            &self.ssh,
            self.ssh_identity.as_deref(),
            &["sh", "-c", &webui],
        )
        .await?
            != 0
        {
            return Err("open-webui pods not Ready".into());
        }

        Ok(())
    }

    pub async fn down(self) -> Result<(), String> {
        let _ = stop_microvm(&self.handle).await;
        let _ = tokio::fs::remove_dir_all(&self.workspace).await;
        Ok(())
    }
}

fn port_offset() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qemu_available;
    use crate::vm_stack::{load_or_generate_ssh, DEBIAN_12_CLOUD_AMD64};
    use tempfile::tempdir;

    fn k3s_stack_test_enabled() -> bool {
        std::env::var("INFRZEUG_K3S_STACK_TEST")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    #[test]
    fn split_log_poll_returns_log_and_total_line_count() {
        let output = "[k3s-stack] install k3s\n[k3s-stack] wait for node\n__infrazeug_lines__42\n";

        let (log, lines) = split_log_poll(output).expect("poll marker");

        assert_eq!(log, "[k3s-stack] install k3s\n[k3s-stack] wait for node\n");
        assert_eq!(lines, 42);
    }

    #[test]
    fn split_log_poll_ignores_ssh_stderr_after_marker() {
        let output = "[k3s-stack] install k3s\n__infrazeug_lines__42\nWarning: host key\n";

        let (log, lines) = split_log_poll(output).expect("poll marker");

        assert_eq!(log, "[k3s-stack] install k3s\n");
        assert_eq!(lines, 42);
    }

    /// Single Debian microVM running k3s + Cilium + OpenEBS + CNPG + Open WebUI.
    ///
    /// ```no_run
    /// INFRZEUG_K3S_STACK_TEST=1 cargo test -p infrazeug-emulate-qemu k3s_helm_stack_cilium_openebs_cnpg_openwebui -- --ignored --nocapture
    /// ```
    /// Runs alone: do not combine with other QEMU stacks on the same host (use `--test-threads=1`).
    #[tokio::test]
    #[ignore = "requires Debian cloud qcow2 + qemu + ~8GiB RAM; run with INFRZEUG_K3S_STACK_TEST=1"]
    async fn k3s_helm_stack_cilium_openebs_cnpg_openwebui() {
        if !k3s_stack_test_enabled() {
            eprintln!("skip: set INFRZEUG_K3S_STACK_TEST=1 to run this test");
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

        let tmp = tempdir().expect("tempdir");
        let (pubkey, identity) = load_or_generate_ssh(tmp.path()).expect("ssh key");
        let user = std::env::var("INFRZEUG_QEMU_SSH_USER").unwrap_or_else(|_| "debian".into());
        let host = QemuHost::new(tmp.path().to_path_buf());

        eprintln!("debian cloud image: {}", image.display());
        eprintln!("booting k3s lab VM (bootstrap may take 20–40 minutes)…");

        let stack = K3sHelmStack::up(&host, &image, &pubkey, &user, identity)
            .await
            .expect("k3s stack should start");

        let result = stack.verify().await;

        stack.down().await.expect("k3s stack teardown");

        result.expect("k3s helm stack health checks");
    }
}
