//! Ensure a UniFi network (LAN / VLAN-only network) exists and matches a small set
//! of managed fields.

use crate::client::UnifiClientSource;
use async_trait::async_trait;
use infrazeug_ext_unifi_api::network::NetworkConf;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_NETWORK: &str = "unifi.ensure_network";

/// Tier-1 method: ensure a UniFi network.
pub type EnsureNetwork = EnsureResource<NetworkResource>;

/// Construct the registrable [`EnsureNetwork`] method for a client source.
pub fn ensure_network(source: UnifiClientSource) -> EnsureNetwork {
    EnsureResource::new(NetworkResource::new(source))
}

fn default_purpose() -> String {
    "corporate".into()
}

/// Desired network — only the managed fields. The network name is the natural key.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureNetworkInput {
    pub name: String,
    /// `corporate`, `vlan-only`, or `guest`. Defaults to `corporate`.
    #[serde(default = "default_purpose")]
    pub purpose: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlan: Option<u16>,
    /// Defaults to `vlan.is_some()` on create when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlan_enabled: Option<bool>,
    /// Gateway CIDR for routed networks (e.g. `10.0.20.1/24`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_subnet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dhcpd_enabled: Option<bool>,
    /// Defaults to `true` on create when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Observed network — the managed fields plus the controller id, captured for
/// downstream nodes (e.g. a WLAN that binds onto `network_id`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureNetworkOutput {
    pub network_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlan: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// A UniFi network as an acquirable resource.
#[derive(Clone)]
pub struct NetworkResource {
    source: UnifiClientSource,
}

impl NetworkResource {
    pub fn new(source: UnifiClientSource) -> Self {
        Self { source }
    }

    /// Fetch the full controller object by name (needed for read-modify-write).
    async fn find(&self, ctx: &ResourceCtx, name: &str) -> ResourceResult<Option<NetworkConf>> {
        let client = self.source.client(ctx).await?;
        let networks = client.networks().await.map_err(ResourceError::provider)?;
        Ok(networks.into_iter().find(|n| n.name == name))
    }
}

fn to_output(conf: NetworkConf) -> Option<EnsureNetworkOutput> {
    let id = conf.id?;
    Some(EnsureNetworkOutput {
        network_id: id,
        name: conf.name,
        purpose: conf.purpose,
        vlan: conf.vlan,
        enabled: conf.enabled,
    })
}

#[async_trait]
impl Resource for NetworkResource {
    type Spec = EnsureNetworkInput;
    type State = EnsureNetworkOutput;

    fn kind(&self) -> &'static str {
        ENSURE_NETWORK
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        Ok(self.find(ctx, &spec.name).await?.and_then(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let body = NetworkConf {
            name: spec.name.clone(),
            purpose: Some(spec.purpose.clone()),
            vlan: spec.vlan,
            vlan_enabled: Some(spec.vlan_enabled.unwrap_or(spec.vlan.is_some())),
            ip_subnet: spec.ip_subnet.clone(),
            dhcpd_enabled: spec.dhcpd_enabled,
            enabled: Some(spec.enabled.unwrap_or(true)),
            ..Default::default()
        };
        let created = client
            .create_network(&body)
            .await
            .map_err(ResourceError::provider)?;
        to_output(created).ok_or_else(|| ResourceError::provider("created network has no id"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if current.purpose.as_deref() != Some(spec.purpose.as_str()) {
            diffs.push(format!(
                "purpose {:?} → {:?}",
                current.purpose, spec.purpose
            ));
        }
        if let Some(vlan) = spec.vlan {
            if current.vlan != Some(vlan) {
                diffs.push(format!("vlan {:?} → {}", current.vlan, vlan));
            }
        }
        if let Some(enabled) = spec.enabled {
            if current.enabled != Some(enabled) {
                diffs.push(format!("enabled {:?} → {}", current.enabled, enabled));
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
        // Re-read the full object so unmanaged fields survive the PUT.
        let mut conf = self
            .find(ctx, &spec.name)
            .await?
            .ok_or_else(|| ResourceError::provider("network disappeared before reconcile"))?;
        conf.purpose = Some(spec.purpose.clone());
        if spec.vlan.is_some() {
            conf.vlan = spec.vlan;
            conf.vlan_enabled = Some(spec.vlan_enabled.unwrap_or(true));
        }
        if spec.ip_subnet.is_some() {
            conf.ip_subnet = spec.ip_subnet.clone();
        }
        if spec.dhcpd_enabled.is_some() {
            conf.dhcpd_enabled = spec.dhcpd_enabled;
        }
        if let Some(enabled) = spec.enabled {
            conf.enabled = Some(enabled);
        }
        let updated = client
            .update_network(&current.network_id, &conf)
            .await
            .map_err(ResourceError::provider)?;
        to_output(updated).ok_or_else(|| ResourceError::provider("updated network has no id"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> NetworkResource {
        NetworkResource::new(UnifiClientSource::vault(
            "https://unifi.local",
            "unifi.vault",
        ))
    }

    fn current() -> EnsureNetworkOutput {
        EnsureNetworkOutput {
            network_id: "net-1".into(),
            name: "iot".into(),
            purpose: Some("vlan-only".into()),
            vlan: Some(20),
            enabled: Some(true),
        }
    }

    #[test]
    fn matching_spec_is_in_sync() {
        let spec = EnsureNetworkInput {
            name: "iot".into(),
            purpose: "vlan-only".into(),
            vlan: Some(20),
            ..Default::default()
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_vlan_drifts() {
        let spec = EnsureNetworkInput {
            name: "iot".into(),
            purpose: "vlan-only".into(),
            vlan: Some(30),
            ..Default::default()
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }
}
