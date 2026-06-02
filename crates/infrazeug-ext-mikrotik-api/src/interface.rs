//! Interfaces (`/interface`).

use crate::client::MikrotikClient;
use crate::error::Result;
use std::collections::HashMap;

const PATH: &str = "/interface";

/// A RouterOS interface (ether, vlan, bridge, …).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Interface {
    pub id: Option<String>,
    pub name: String,
    pub interface_type: Option<String>,
    pub mtu: Option<String>,
    pub disabled: Option<bool>,
    pub vlan_id: Option<String>,
    pub interface: Option<String>,
    pub extra: HashMap<String, String>,
}

impl Interface {
    fn from_map(mut map: HashMap<String, String>) -> Self {
        let id = map.remove("id").or_else(|| map.remove(".id"));
        let name = map.remove("name").unwrap_or_default();
        let interface_type = map.remove("type");
        let mtu = map.remove("mtu");
        let disabled = map.remove("disabled").map(|v| v == "true" || v == "yes");
        let vlan_id = map.remove("vlan-id");
        let interface = map.remove("interface");
        Self {
            id,
            name,
            interface_type,
            mtu,
            disabled,
            vlan_id,
            interface,
            extra: map,
        }
    }
}

impl MikrotikClient {
    /// List all interfaces.
    pub async fn interfaces(&mut self) -> Result<Vec<Interface>> {
        let rows = self.print(PATH, None, &[]).await?;
        Ok(rows.into_iter().map(Interface::from_map).collect())
    }

    /// List ethernet interfaces only.
    pub async fn ether_interfaces(&mut self) -> Result<Vec<Interface>> {
        let rows = self.print(PATH, None, &[("type", "ether")]).await?;
        Ok(rows.into_iter().map(Interface::from_map).collect())
    }

    /// List VLAN interfaces only.
    pub async fn vlan_interfaces(&mut self) -> Result<Vec<Interface>> {
        let rows = self.print(PATH, None, &[("type", "vlan")]).await?;
        Ok(rows.into_iter().map(Interface::from_map).collect())
    }

    /// Update an interface by `.id`.
    pub async fn set_interface(&mut self, id: &str, attrs: &[(&str, &str)]) -> Result<()> {
        self.set(PATH, id, attrs).await
    }
}
