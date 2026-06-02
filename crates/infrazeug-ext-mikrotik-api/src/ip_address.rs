//! IP addresses (`/ip/address`).

use crate::client::MikrotikClient;
use crate::error::Result;
use std::collections::HashMap;

const PATH: &str = "/ip/address";

/// An IPv4/IPv6 address bound to an interface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IpAddress {
    /// Internal RouterOS id (e.g. `*1`).
    pub id: Option<String>,
    pub address: String,
    pub interface: String,
    pub network: Option<String>,
    pub disabled: Option<bool>,
    /// Unmodelled fields preserved across updates.
    pub extra: HashMap<String, String>,
}

impl IpAddress {
    fn from_map(mut map: HashMap<String, String>) -> Self {
        let id = map.remove("id").or_else(|| map.remove(".id"));
        let address = map.remove("address").unwrap_or_default();
        let interface = map.remove("interface").unwrap_or_default();
        let network = map.remove("network");
        let disabled = map.remove("disabled").map(|v| v == "true" || v == "yes");
        Self {
            id,
            address,
            interface,
            network,
            disabled,
            extra: map,
        }
    }

    fn to_attrs(&self) -> Vec<(&str, &str)> {
        let mut attrs = Vec::new();
        attrs.push(("address", self.address.as_str()));
        attrs.push(("interface", self.interface.as_str()));
        if let Some(ref n) = self.network {
            attrs.push(("network", n.as_str()));
        }
        if let Some(disabled) = self.disabled {
            attrs.push(("disabled", if disabled { "yes" } else { "no" }));
        }
        for (k, v) in &self.extra {
            attrs.push((k.as_str(), v.as_str()));
        }
        attrs
    }
}

impl MikrotikClient {
    /// List all `/ip/address` entries.
    pub async fn ip_addresses(&mut self) -> Result<Vec<IpAddress>> {
        let rows = self.print(PATH, None, &[]).await?;
        Ok(rows.into_iter().map(IpAddress::from_map).collect())
    }

    /// Add an IP address.
    pub async fn add_ip_address(&mut self, addr: &IpAddress) -> Result<IpAddress> {
        let attrs = addr.to_attrs();
        let created = self.add(PATH, &attrs).await?;
        Ok(IpAddress::from_map(created))
    }

    /// Update an IP address by `.id`.
    pub async fn set_ip_address(&mut self, id: &str, addr: &IpAddress) -> Result<()> {
        let attrs = addr.to_attrs();
        self.set(PATH, id, &attrs).await
    }

    /// Remove an IP address by `.id`.
    pub async fn remove_ip_address(&mut self, id: &str) -> Result<()> {
        self.remove(PATH, id).await
    }
}
