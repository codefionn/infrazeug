//! QEMU/KVM virtual machine management (`/nodes/{node}/qemu`).

use crate::client::ProxmoxClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A QEMU VM as returned by the list endpoint (`GET /nodes/{node}/qemu`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct QemuStatus {
    pub vmid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// `running` or `stopped`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Number of virtual CPUs currently configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<f64>,
    /// Maximum memory in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxmem: Option<u64>,
    /// Uptime in seconds (only when running).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime: Option<u64>,
}

/// The flat `key=value` configuration of a VM (`GET /nodes/{node}/qemu/{vmid}/config`).
///
/// Proxmox returns config values with mixed JSON typing (integers for `cores`,
/// strings for `net0`, …), so they are kept as raw values with coercing getters.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QemuConfig {
    #[serde(flatten)]
    pub fields: BTreeMap<String, serde_json::Value>,
}

impl QemuConfig {
    /// Read a field as a string, coercing numbers/bools to their text form.
    pub fn get_str(&self, key: &str) -> Option<String> {
        value_as_string(self.fields.get(key)?)
    }

    /// Read a field as an unsigned integer (parsing numeric strings too).
    pub fn get_u64(&self, key: &str) -> Option<u64> {
        value_as_u64(self.fields.get(key)?)
    }

    pub fn name(&self) -> Option<String> {
        self.get_str("name")
    }

    pub fn cores(&self) -> Option<u64> {
        self.get_u64("cores")
    }

    pub fn sockets(&self) -> Option<u64> {
        self.get_u64("sockets")
    }

    /// Configured memory in MiB.
    pub fn memory(&self) -> Option<u64> {
        self.get_u64("memory")
    }

    /// SHA1 digest of the live config, for optimistic-concurrency updates.
    ///
    /// Pass this back as the `digest` parameter on a config update and Proxmox
    /// rejects the write if the configuration changed in the meantime.
    pub fn digest(&self) -> Option<String> {
        self.get_str("digest")
    }
}

/// Typed parameters for creating a VM (`POST /nodes/{node}/qemu`).
///
/// Common knobs are typed; anything else (disks like `scsi0`, cloud-init keys,
/// `ipconfig0`, …) goes through [`extra`](Self::extra), which is merged verbatim
/// into the form body.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QemuCreate {
    pub vmid: u32,
    pub name: Option<String>,
    pub cores: Option<u32>,
    pub sockets: Option<u32>,
    /// Memory in MiB.
    pub memory: Option<u64>,
    pub ostype: Option<String>,
    pub scsihw: Option<String>,
    /// Primary NIC, e.g. `virtio,bridge=vmbr0`.
    pub net0: Option<String>,
    pub boot: Option<String>,
    /// Enable the QEMU guest agent.
    pub agent: Option<bool>,
    /// Start the VM right after creation.
    pub start: Option<bool>,
    pub pool: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    /// Arbitrary additional `key=value` config (disks, cloud-init, …).
    pub extra: BTreeMap<String, String>,
}

impl QemuCreate {
    /// Flatten into the form parameters Proxmox expects.
    pub fn to_params(&self) -> BTreeMap<String, String> {
        let mut p = BTreeMap::new();
        p.insert("vmid".into(), self.vmid.to_string());
        insert_opt(&mut p, "name", self.name.as_ref());
        insert_opt(&mut p, "cores", self.cores.map(|v| v.to_string()).as_ref());
        insert_opt(
            &mut p,
            "sockets",
            self.sockets.map(|v| v.to_string()).as_ref(),
        );
        insert_opt(
            &mut p,
            "memory",
            self.memory.map(|v| v.to_string()).as_ref(),
        );
        insert_opt(&mut p, "ostype", self.ostype.as_ref());
        insert_opt(&mut p, "scsihw", self.scsihw.as_ref());
        insert_opt(&mut p, "net0", self.net0.as_ref());
        insert_opt(&mut p, "boot", self.boot.as_ref());
        insert_opt(&mut p, "agent", self.agent.map(bool_param).as_ref());
        insert_opt(&mut p, "start", self.start.map(bool_param).as_ref());
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
    /// `GET /nodes/{node}/qemu` — list VMs on a node.
    pub async fn qemu_list(&self, node: &str) -> Result<Vec<QemuStatus>> {
        self.get(&format!("/nodes/{}/qemu", self.encode(node)))
            .await
    }

    /// `GET /nodes/{node}/qemu/{vmid}/config` — read a VM's configuration.
    pub async fn qemu_config(&self, node: &str, vmid: u32) -> Result<QemuConfig> {
        self.get(&format!(
            "/nodes/{}/qemu/{}/config",
            self.encode(node),
            vmid
        ))
        .await
    }

    /// `POST /nodes/{node}/qemu` — create a VM. Returns the task UPID.
    pub async fn qemu_create(&self, node: &str, create: &QemuCreate) -> Result<String> {
        let upid: Option<String> = self
            .post_form(
                &format!("/nodes/{}/qemu", self.encode(node)),
                &create.to_params(),
            )
            .await?;
        Ok(upid.unwrap_or_default())
    }

    /// `PUT /nodes/{node}/qemu/{vmid}/config` — update a VM's configuration.
    pub async fn qemu_update_config(
        &self,
        node: &str,
        vmid: u32,
        params: &BTreeMap<String, String>,
    ) -> Result<()> {
        let _: serde_json::Value = self
            .put_form(
                &format!("/nodes/{}/qemu/{}/config", self.encode(node), vmid),
                params,
            )
            .await?;
        Ok(())
    }

    /// `DELETE /nodes/{node}/qemu/{vmid}` — destroy a VM.
    pub async fn qemu_delete(&self, node: &str, vmid: u32) -> Result<()> {
        self.delete(&format!("/nodes/{}/qemu/{}", self.encode(node), vmid))
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

pub(crate) fn value_as_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

pub(crate) fn value_as_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(n) => n.as_u64(),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_flattens_params() {
        let mut create = QemuCreate {
            vmid: 100,
            name: Some("web-1".into()),
            cores: Some(2),
            memory: Some(2048),
            agent: Some(true),
            ..Default::default()
        };
        create.extra.insert("scsi0".into(), "local-lvm:32".into());
        let p = create.to_params();
        assert_eq!(p.get("vmid").unwrap(), "100");
        assert_eq!(p.get("name").unwrap(), "web-1");
        assert_eq!(p.get("cores").unwrap(), "2");
        assert_eq!(p.get("memory").unwrap(), "2048");
        assert_eq!(p.get("agent").unwrap(), "1");
        assert_eq!(p.get("scsi0").unwrap(), "local-lvm:32");
        assert!(!p.contains_key("sockets"));
    }

    #[test]
    fn config_coerces_mixed_types() {
        let body = r#"{"name":"web-1","cores":4,"memory":"2048","sockets":1}"#;
        let cfg: QemuConfig = serde_json::from_str(body).unwrap();
        assert_eq!(cfg.name().as_deref(), Some("web-1"));
        assert_eq!(cfg.cores(), Some(4));
        assert_eq!(cfg.memory(), Some(2048));
        assert_eq!(cfg.sockets(), Some(1));
    }

    #[test]
    fn config_reads_digest() {
        let body = r#"{"name":"web-1","digest":"a1b2c3d4e5f6"}"#;
        let cfg: QemuConfig = serde_json::from_str(body).unwrap();
        assert_eq!(cfg.digest().as_deref(), Some("a1b2c3d4e5f6"));
    }
}
