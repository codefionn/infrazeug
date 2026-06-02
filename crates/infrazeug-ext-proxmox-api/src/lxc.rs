//! LXC container management (`/nodes/{node}/lxc`).

use crate::client::ProxmoxClient;
use crate::error::Result;
use crate::qemu::{value_as_string, value_as_u64};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An LXC container as returned by the list endpoint (`GET /nodes/{node}/lxc`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct LxcStatus {
    pub vmid: u32,
    /// Container hostname (`name` in the list payload).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `running` or `stopped`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<f64>,
    /// Maximum memory in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxmem: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<u64>,
}

/// The flat `key=value` configuration of a container
/// (`GET /nodes/{node}/lxc/{vmid}/config`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LxcConfig {
    #[serde(flatten)]
    pub fields: BTreeMap<String, serde_json::Value>,
}

impl LxcConfig {
    pub fn get_str(&self, key: &str) -> Option<String> {
        value_as_string(self.fields.get(key)?)
    }

    pub fn get_u64(&self, key: &str) -> Option<u64> {
        value_as_u64(self.fields.get(key)?)
    }

    pub fn hostname(&self) -> Option<String> {
        self.get_str("hostname")
    }

    pub fn cores(&self) -> Option<u64> {
        self.get_u64("cores")
    }

    /// Configured memory in MiB.
    pub fn memory(&self) -> Option<u64> {
        self.get_u64("memory")
    }

    /// Configured swap in MiB.
    pub fn swap(&self) -> Option<u64> {
        self.get_u64("swap")
    }

    /// SHA1 digest of the live config, for optimistic-concurrency updates.
    ///
    /// Pass this back as the `digest` parameter on a config update and Proxmox
    /// rejects the write if the configuration changed in the meantime.
    pub fn digest(&self) -> Option<String> {
        self.get_str("digest")
    }
}

/// Typed parameters for creating a container (`POST /nodes/{node}/lxc`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LxcCreate {
    pub vmid: u32,
    /// Template volume, e.g. `local:vztmpl/debian-12-standard_12.7-1_amd64.tar.zst`.
    pub ostemplate: String,
    pub hostname: Option<String>,
    pub cores: Option<u32>,
    /// Memory in MiB.
    pub memory: Option<u64>,
    /// Swap in MiB.
    pub swap: Option<u64>,
    /// Root filesystem, e.g. `local-lvm:8`.
    pub rootfs: Option<String>,
    /// Primary NIC, e.g. `name=eth0,bridge=vmbr0,ip=dhcp`.
    pub net0: Option<String>,
    /// Default storage for the rootfs when `rootfs` is not given.
    pub storage: Option<String>,
    pub unprivileged: Option<bool>,
    /// Start the container right after creation.
    pub start: Option<bool>,
    /// Root password (maps to the `password` parameter).
    pub password: Option<String>,
    /// Authorized SSH keys (maps to the `ssh-public-keys` parameter).
    pub ssh_public_keys: Option<String>,
    pub pool: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    /// Arbitrary additional `key=value` config (extra mounts, features, …).
    pub extra: BTreeMap<String, String>,
}

impl LxcCreate {
    /// Flatten into the form parameters Proxmox expects.
    pub fn to_params(&self) -> BTreeMap<String, String> {
        let mut p = BTreeMap::new();
        p.insert("vmid".into(), self.vmid.to_string());
        p.insert("ostemplate".into(), self.ostemplate.clone());
        insert_opt(&mut p, "hostname", self.hostname.as_ref());
        insert_opt(&mut p, "cores", self.cores.map(|v| v.to_string()).as_ref());
        insert_opt(
            &mut p,
            "memory",
            self.memory.map(|v| v.to_string()).as_ref(),
        );
        insert_opt(&mut p, "swap", self.swap.map(|v| v.to_string()).as_ref());
        insert_opt(&mut p, "rootfs", self.rootfs.as_ref());
        insert_opt(&mut p, "net0", self.net0.as_ref());
        insert_opt(&mut p, "storage", self.storage.as_ref());
        insert_opt(
            &mut p,
            "unprivileged",
            self.unprivileged.map(bool_param).as_ref(),
        );
        insert_opt(&mut p, "start", self.start.map(bool_param).as_ref());
        insert_opt(&mut p, "password", self.password.as_ref());
        insert_opt(&mut p, "ssh-public-keys", self.ssh_public_keys.as_ref());
        insert_opt(&mut p, "pool", self.pool.as_ref());
        insert_opt(&mut p, "description", self.description.as_ref());
        insert_opt(&mut p, "tags", self.tags.as_ref());
        for (k, v) in &self.extra {
            p.insert(k.clone(), v.clone());
        }
        p
    }
}

impl ProxmoxClient {
    /// `GET /nodes/{node}/lxc` — list containers on a node.
    pub async fn lxc_list(&self, node: &str) -> Result<Vec<LxcStatus>> {
        self.get(&format!("/nodes/{}/lxc", self.encode(node))).await
    }

    /// `GET /nodes/{node}/lxc/{vmid}/config` — read a container's configuration.
    pub async fn lxc_config(&self, node: &str, vmid: u32) -> Result<LxcConfig> {
        self.get(&format!("/nodes/{}/lxc/{}/config", self.encode(node), vmid))
            .await
    }

    /// `POST /nodes/{node}/lxc` — create a container. Returns the task UPID.
    pub async fn lxc_create(&self, node: &str, create: &LxcCreate) -> Result<String> {
        let upid: Option<String> = self
            .post_form(
                &format!("/nodes/{}/lxc", self.encode(node)),
                &create.to_params(),
            )
            .await?;
        Ok(upid.unwrap_or_default())
    }

    /// `PUT /nodes/{node}/lxc/{vmid}/config` — update a container's configuration.
    pub async fn lxc_update_config(
        &self,
        node: &str,
        vmid: u32,
        params: &BTreeMap<String, String>,
    ) -> Result<()> {
        let _: serde_json::Value = self
            .put_form(
                &format!("/nodes/{}/lxc/{}/config", self.encode(node), vmid),
                params,
            )
            .await?;
        Ok(())
    }

    /// `DELETE /nodes/{node}/lxc/{vmid}` — destroy a container.
    pub async fn lxc_delete(&self, node: &str, vmid: u32) -> Result<()> {
        self.delete(&format!("/nodes/{}/lxc/{}", self.encode(node), vmid))
            .await
    }
}

fn insert_opt(map: &mut BTreeMap<String, String>, key: &str, value: Option<&String>) {
    if let Some(value) = value {
        map.insert(key.to_string(), value.clone());
    }
}

fn bool_param(value: bool) -> String {
    if value {
        "1".into()
    } else {
        "0".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_requires_template_and_flattens() {
        let create = LxcCreate {
            vmid: 200,
            ostemplate: "local:vztmpl/debian-12.tar.zst".into(),
            hostname: Some("ct-1".into()),
            cores: Some(1),
            memory: Some(512),
            unprivileged: Some(true),
            ssh_public_keys: Some("ssh-ed25519 AAAA".into()),
            ..Default::default()
        };
        let p = create.to_params();
        assert_eq!(p.get("vmid").unwrap(), "200");
        assert_eq!(
            p.get("ostemplate").unwrap(),
            "local:vztmpl/debian-12.tar.zst"
        );
        assert_eq!(p.get("hostname").unwrap(), "ct-1");
        assert_eq!(p.get("unprivileged").unwrap(), "1");
        assert_eq!(p.get("ssh-public-keys").unwrap(), "ssh-ed25519 AAAA");
    }

    #[test]
    fn config_reads_hostname_and_resources() {
        let body = r#"{"hostname":"ct-1","cores":2,"memory":1024,"swap":"512"}"#;
        let cfg: LxcConfig = serde_json::from_str(body).unwrap();
        assert_eq!(cfg.hostname().as_deref(), Some("ct-1"));
        assert_eq!(cfg.cores(), Some(2));
        assert_eq!(cfg.memory(), Some(1024));
        assert_eq!(cfg.swap(), Some(512));
    }
}
