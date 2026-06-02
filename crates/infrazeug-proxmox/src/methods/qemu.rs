//! Ensure a Proxmox QEMU/KVM virtual machine exists (and matches CPU/memory/name).

use crate::client::ProxmoxClientSource;
use crate::methods::wait::await_task;
use async_trait::async_trait;
use infrazeug_ext_proxmox_api::qemu::{QemuConfig, QemuCreate, QemuStatus};
use infrazeug_ext_proxmox_api::ProxmoxClient;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ENSURE_QEMU: &str = "proxmox.ensure_qemu";

pub type EnsureQemu = EnsureResource<QemuResource>;

pub fn ensure_qemu(source: ProxmoxClientSource) -> EnsureQemu {
    EnsureResource::new(QemuResource::new(source))
}

/// Desired state of a QEMU VM, keyed by `node` + `vmid`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureQemuInput {
    /// Proxmox node the VM lives on, e.g. `pve`.
    pub node: String,
    /// Cluster-unique numeric VM id.
    pub vmid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sockets: Option<u32>,
    /// Memory in MiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ostype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scsihw: Option<String>,
    /// Primary NIC, e.g. `virtio,bridge=vmbr0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net0: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<bool>,
    /// Start the VM right after creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    /// Arbitrary additional `key=value` config (disks like `scsi0`, cloud-init, …).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
    /// How long to wait for the asynchronous create task to finish, in seconds.
    /// `None` waits with the default timeout; `Some(0)` returns immediately
    /// (fire-and-forget) without polling the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_timeout_secs: Option<u64>,
}

/// Observed/created VM state (captured for downstream nodes & vault writes).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureQemuOutput {
    pub vmid: u32,
    pub node: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sockets: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,
}

#[derive(Clone)]
pub struct QemuResource {
    source: ProxmoxClientSource,
}

impl QemuResource {
    pub fn new(source: ProxmoxClientSource) -> Self {
        Self { source }
    }
}

impl EnsureQemuInput {
    fn to_create(&self) -> QemuCreate {
        QemuCreate {
            vmid: self.vmid,
            name: self.name.clone(),
            cores: self.cores,
            sockets: self.sockets,
            memory: self.memory,
            ostype: self.ostype.clone(),
            scsihw: self.scsihw.clone(),
            net0: self.net0.clone(),
            boot: self.boot.clone(),
            agent: self.agent,
            start: self.start,
            pool: self.pool.clone(),
            description: self.description.clone(),
            tags: self.tags.clone(),
            extra: self.extra.clone(),
        }
    }
}

/// Merge list status + (optional) config detail into the captured output.
fn build_output(
    node: &str,
    status: Option<&QemuStatus>,
    config: Option<&QemuConfig>,
) -> EnsureQemuOutput {
    EnsureQemuOutput {
        vmid: status.map(|s| s.vmid).unwrap_or_default(),
        node: node.to_string(),
        name: config
            .and_then(QemuConfig::name)
            .or_else(|| status.and_then(|s| s.name.clone())),
        status: status.and_then(|s| s.status.clone()),
        cores: config.and_then(QemuConfig::cores),
        sockets: config.and_then(QemuConfig::sockets),
        memory: config.and_then(QemuConfig::memory),
    }
}

async fn read_qemu(
    client: &ProxmoxClient,
    node: &str,
    vmid: u32,
) -> ResourceResult<Option<EnsureQemuOutput>> {
    let list = client
        .qemu_list(node)
        .await
        .map_err(ResourceError::provider)?;
    let Some(status) = list.into_iter().find(|s| s.vmid == vmid) else {
        return Ok(None);
    };
    // Enrich with the config so drift on cores/memory/name can be detected; tolerate
    // a config read failure (e.g. mid-creation) by returning list-only fields.
    let config = client.qemu_config(node, vmid).await.ok();
    Ok(Some(build_output(node, Some(&status), config.as_ref())))
}

#[async_trait]
impl Resource for QemuResource {
    type Spec = EnsureQemuInput;
    type State = EnsureQemuOutput;

    fn kind(&self) -> &'static str {
        ENSURE_QEMU
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        read_qemu(&client, &spec.node, spec.vmid).await
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let upid = client
            .qemu_create(&spec.node, &spec.to_create())
            .await
            .map_err(ResourceError::provider)?;
        // Creation runs as an async node task; block until it finishes so the
        // re-read below sees the live VM (and so failures surface here, not later).
        await_task(&client, &spec.node, &upid, spec.wait_timeout_secs).await?;
        // Re-read so captured outputs reflect the live VM; fall back to the spec.
        if let Some(state) = read_qemu(&client, &spec.node, spec.vmid).await? {
            return Ok(state);
        }
        Ok(EnsureQemuOutput {
            vmid: spec.vmid,
            node: spec.node.clone(),
            name: spec.name.clone(),
            status: None,
            cores: spec.cores.map(u64::from),
            sockets: spec.sockets.map(u64::from),
            memory: spec.memory,
        })
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if let Some(name) = &spec.name {
            if current.name.as_deref() != Some(name.as_str()) {
                diffs.push(format!("name {:?} → {:?}", current.name, name));
            }
        }
        if let Some(cores) = spec.cores {
            if current.cores != Some(u64::from(cores)) {
                diffs.push(format!("cores {:?} → {}", current.cores, cores));
            }
        }
        if let Some(sockets) = spec.sockets {
            if current.sockets != Some(u64::from(sockets)) {
                diffs.push(format!("sockets {:?} → {}", current.sockets, sockets));
            }
        }
        if let Some(memory) = spec.memory {
            if current.memory != Some(memory) {
                diffs.push(format!("memory {:?} → {}", current.memory, memory));
            }
        }
        if diffs.is_empty() {
            Drift::InSync
        } else {
            Drift::Drifted(diffs.join(", "))
        }
    }

    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let mut params = BTreeMap::new();
        if let Some(name) = &spec.name {
            params.insert("name".to_string(), name.clone());
        }
        if let Some(cores) = spec.cores {
            params.insert("cores".to_string(), cores.to_string());
        }
        if let Some(sockets) = spec.sockets {
            params.insert("sockets".to_string(), sockets.to_string());
        }
        if let Some(memory) = spec.memory {
            params.insert("memory".to_string(), memory.to_string());
        }
        if params.is_empty() {
            return Ok(current);
        }
        // Optimistic concurrency: bind the edit to the config we just observed by
        // sending its SHA1 `digest`. If anything changed it in the meantime (an
        // admin, another run), Proxmox rejects the update instead of silently
        // clobbering it. Best-effort: if the digest read fails the PUT would too.
        if let Ok(config) = client.qemu_config(&spec.node, spec.vmid).await {
            if let Some(digest) = config.digest() {
                params.insert("digest".to_string(), digest);
            }
        }
        client
            .qemu_update_config(&spec.node, spec.vmid, &params)
            .await
            .map_err(ResourceError::provider)?;
        Ok(read_qemu(&client, &spec.node, spec.vmid)
            .await?
            .unwrap_or(current))
    }
}
