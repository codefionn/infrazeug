//! Ensure a Proxmox LXC container exists (and matches CPU/memory/hostname).

use crate::client::ProxmoxClientSource;
use crate::methods::wait::await_task;
use async_trait::async_trait;
use infrazeug_ext_proxmox_api::lxc::{LxcConfig, LxcCreate, LxcStatus};
use infrazeug_ext_proxmox_api::ProxmoxClient;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ENSURE_LXC: &str = "proxmox.ensure_lxc";

pub type EnsureLxc = EnsureResource<LxcResource>;

pub fn ensure_lxc(source: ProxmoxClientSource) -> EnsureLxc {
    EnsureResource::new(LxcResource::new(source))
}

/// Desired state of an LXC container, keyed by `node` + `vmid`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureLxcInput {
    /// Proxmox node the container lives on, e.g. `pve`.
    pub node: String,
    /// Cluster-unique numeric container id.
    pub vmid: u32,
    /// Template volume, e.g. `local:vztmpl/debian-12-standard_12.7-1_amd64.tar.zst`.
    /// Required on create; ignored once the container exists.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ostemplate: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores: Option<u32>,
    /// Memory in MiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,
    /// Swap in MiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap: Option<u64>,
    /// Root filesystem, e.g. `local-lvm:8`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rootfs: Option<String>,
    /// Primary NIC, e.g. `name=eth0,bridge=vmbr0,ip=dhcp`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net0: Option<String>,
    /// Default storage for the rootfs when `rootfs` is not given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unprivileged: Option<bool>,
    /// Start the container right after creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<bool>,
    /// Root password (maps to the `password` create parameter).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Authorized SSH keys (maps to `ssh-public-keys`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_public_keys: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    /// Arbitrary additional `key=value` config (mount points, features, …).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
    /// How long to wait for the asynchronous create task to finish, in seconds.
    /// `None` waits with the default timeout; `Some(0)` returns immediately
    /// (fire-and-forget) without polling the task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_timeout_secs: Option<u64>,
}

/// Observed/created container state (captured for downstream nodes & vault writes).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureLxcOutput {
    pub vmid: u32,
    pub node: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap: Option<u64>,
}

#[derive(Clone)]
pub struct LxcResource {
    source: ProxmoxClientSource,
}

impl LxcResource {
    pub fn new(source: ProxmoxClientSource) -> Self {
        Self { source }
    }
}

impl EnsureLxcInput {
    fn to_create(&self) -> LxcCreate {
        LxcCreate {
            vmid: self.vmid,
            ostemplate: self.ostemplate.clone(),
            hostname: self.hostname.clone(),
            cores: self.cores,
            memory: self.memory,
            swap: self.swap,
            rootfs: self.rootfs.clone(),
            net0: self.net0.clone(),
            storage: self.storage.clone(),
            unprivileged: self.unprivileged,
            start: self.start,
            password: self.password.clone(),
            ssh_public_keys: self.ssh_public_keys.clone(),
            pool: self.pool.clone(),
            description: self.description.clone(),
            tags: self.tags.clone(),
            extra: self.extra.clone(),
        }
    }
}

fn build_output(
    node: &str,
    status: Option<&LxcStatus>,
    config: Option<&LxcConfig>,
) -> EnsureLxcOutput {
    EnsureLxcOutput {
        vmid: status.map(|s| s.vmid).unwrap_or_default(),
        node: node.to_string(),
        hostname: config
            .and_then(LxcConfig::hostname)
            .or_else(|| status.and_then(|s| s.name.clone())),
        status: status.and_then(|s| s.status.clone()),
        cores: config.and_then(LxcConfig::cores),
        memory: config.and_then(LxcConfig::memory),
        swap: config.and_then(LxcConfig::swap),
    }
}

async fn read_lxc(
    client: &ProxmoxClient,
    node: &str,
    vmid: u32,
) -> ResourceResult<Option<EnsureLxcOutput>> {
    let list = client
        .lxc_list(node)
        .await
        .map_err(ResourceError::provider)?;
    let Some(status) = list.into_iter().find(|s| s.vmid == vmid) else {
        return Ok(None);
    };
    let config = client.lxc_config(node, vmid).await.ok();
    Ok(Some(build_output(node, Some(&status), config.as_ref())))
}

#[async_trait]
impl Resource for LxcResource {
    type Spec = EnsureLxcInput;
    type State = EnsureLxcOutput;

    fn kind(&self) -> &'static str {
        ENSURE_LXC
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        read_lxc(&client, &spec.node, spec.vmid).await
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        if spec.ostemplate.trim().is_empty() {
            return Err(ResourceError::provider(
                "proxmox.ensure_lxc requires `ostemplate` to create a container",
            ));
        }
        let client = self.source.client(ctx).await?;
        let upid = client
            .lxc_create(&spec.node, &spec.to_create())
            .await
            .map_err(ResourceError::provider)?;
        // Creation runs as an async node task; block until it finishes so the
        // re-read below sees the live container (and failures surface here).
        await_task(&client, &spec.node, &upid, spec.wait_timeout_secs).await?;
        if let Some(state) = read_lxc(&client, &spec.node, spec.vmid).await? {
            return Ok(state);
        }
        Ok(EnsureLxcOutput {
            vmid: spec.vmid,
            node: spec.node.clone(),
            hostname: spec.hostname.clone(),
            status: None,
            cores: spec.cores.map(u64::from),
            memory: spec.memory,
            swap: spec.swap,
        })
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if let Some(hostname) = &spec.hostname {
            if current.hostname.as_deref() != Some(hostname.as_str()) {
                diffs.push(format!("hostname {:?} → {:?}", current.hostname, hostname));
            }
        }
        if let Some(cores) = spec.cores {
            if current.cores != Some(u64::from(cores)) {
                diffs.push(format!("cores {:?} → {}", current.cores, cores));
            }
        }
        if let Some(memory) = spec.memory {
            if current.memory != Some(memory) {
                diffs.push(format!("memory {:?} → {}", current.memory, memory));
            }
        }
        if let Some(swap) = spec.swap {
            if current.swap != Some(swap) {
                diffs.push(format!("swap {:?} → {}", current.swap, swap));
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
        if let Some(hostname) = &spec.hostname {
            params.insert("hostname".to_string(), hostname.clone());
        }
        if let Some(cores) = spec.cores {
            params.insert("cores".to_string(), cores.to_string());
        }
        if let Some(memory) = spec.memory {
            params.insert("memory".to_string(), memory.to_string());
        }
        if let Some(swap) = spec.swap {
            params.insert("swap".to_string(), swap.to_string());
        }
        if params.is_empty() {
            return Ok(current);
        }
        // Optimistic concurrency: bind the edit to the config we just observed by
        // sending its SHA1 `digest`. If anything changed it in the meantime (an
        // admin, another run), Proxmox rejects the update instead of silently
        // clobbering it. Best-effort: if the digest read fails the PUT would too.
        if let Ok(config) = client.lxc_config(&spec.node, spec.vmid).await {
            if let Some(digest) = config.digest() {
                params.insert("digest".to_string(), digest);
            }
        }
        client
            .lxc_update_config(&spec.node, spec.vmid, &params)
            .await
            .map_err(ResourceError::provider)?;
        Ok(read_lxc(&client, &spec.node, spec.vmid)
            .await?
            .unwrap_or(current))
    }
}
