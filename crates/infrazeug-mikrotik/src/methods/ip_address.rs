//! Ensure a RouterOS `/ip/address` entry exists and matches managed fields.

use crate::client::MikrotikClientSource;
use async_trait::async_trait;
use infrazeug_ext_mikrotik_api::ip_address::IpAddress;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_IP_ADDRESS: &str = "mikrotik.ensure_ip_address";

/// Tier-1 method: ensure an IP address on an interface.
pub type EnsureIpAddress = EnsureResource<IpAddressResource>;

/// Construct the registrable [`EnsureIpAddress`] method for a client source.
pub fn ensure_ip_address(source: MikrotikClientSource) -> EnsureIpAddress {
    EnsureResource::new(IpAddressResource::new(source))
}

/// Desired IP address. Natural key: `address` + `interface`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureIpAddressInput {
    /// Address with prefix, e.g. `10.0.0.1/24`.
    pub address: String,
    pub interface: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    /// Defaults to `false` (enabled) on create when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

/// Observed IP address — managed fields plus the RouterOS `.id`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureIpAddressOutput {
    pub address_id: String,
    pub address: String,
    pub interface: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

#[derive(Clone)]
pub struct IpAddressResource {
    source: MikrotikClientSource,
}

impl IpAddressResource {
    pub fn new(source: MikrotikClientSource) -> Self {
        Self { source }
    }

    async fn find(
        &self,
        ctx: &ResourceCtx,
        address: &str,
        interface: &str,
    ) -> ResourceResult<Option<IpAddress>> {
        let params = self.source.params(ctx).await?;
        let mut client = params.connect().await.map_err(ResourceError::provider)?;
        let addrs = client
            .ip_addresses()
            .await
            .map_err(ResourceError::provider)?;
        Ok(addrs
            .into_iter()
            .find(|a| a.address == address && a.interface == interface))
    }
}

fn to_output(addr: IpAddress) -> Option<EnsureIpAddressOutput> {
    let id = addr.id?;
    Some(EnsureIpAddressOutput {
        address_id: id,
        address: addr.address,
        interface: addr.interface,
        network: addr.network,
        disabled: addr.disabled,
    })
}

#[async_trait]
impl Resource for IpAddressResource {
    type Spec = EnsureIpAddressInput;
    type State = EnsureIpAddressOutput;

    fn kind(&self) -> &'static str {
        ENSURE_IP_ADDRESS
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        Ok(self
            .find(ctx, &spec.address, &spec.interface)
            .await?
            .and_then(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let params = self.source.params(ctx).await?;
        let mut client = params.connect().await.map_err(ResourceError::provider)?;
        let body = IpAddress {
            address: spec.address.clone(),
            interface: spec.interface.clone(),
            network: spec.network.clone(),
            disabled: Some(spec.disabled.unwrap_or(false)),
            ..Default::default()
        };
        let created = client
            .add_ip_address(&body)
            .await
            .map_err(ResourceError::provider)?;
        to_output(created).ok_or_else(|| ResourceError::provider("created address has no id"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if current.address != spec.address {
            diffs.push(format!(
                "address {:?} → {:?}",
                current.address, spec.address
            ));
        }
        if current.interface != spec.interface {
            diffs.push(format!(
                "interface {:?} → {:?}",
                current.interface, spec.interface
            ));
        }
        if let Some(ref network) = spec.network {
            if current.network.as_deref() != Some(network.as_str()) {
                diffs.push(format!("network {:?} → {:?}", current.network, network));
            }
        }
        if let Some(disabled) = spec.disabled {
            if current.disabled != Some(disabled) {
                diffs.push(format!("disabled {:?} → {}", current.disabled, disabled));
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
        let params = self.source.params(ctx).await?;
        let mut client = params.connect().await.map_err(ResourceError::provider)?;
        let mut addr = self
            .find(ctx, &spec.address, &spec.interface)
            .await?
            .ok_or_else(|| ResourceError::provider("address disappeared before reconcile"))?;
        addr.network = spec.network.clone();
        if let Some(disabled) = spec.disabled {
            addr.disabled = Some(disabled);
        }
        client
            .set_ip_address(&current.address_id, &addr)
            .await
            .map_err(ResourceError::provider)?;
        Ok(EnsureIpAddressOutput {
            address_id: current.address_id,
            address: spec.address.clone(),
            interface: spec.interface.clone(),
            network: spec.network.clone(),
            disabled: spec.disabled.or(current.disabled),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> IpAddressResource {
        IpAddressResource::new(MikrotikClientSource::vault(
            "192.168.88.1",
            "mikrotik.vault",
        ))
    }

    fn current() -> EnsureIpAddressOutput {
        EnsureIpAddressOutput {
            address_id: "*1".into(),
            address: "10.0.0.1/24".into(),
            interface: "bridge".into(),
            network: Some("10.0.0.0".into()),
            disabled: Some(false),
        }
    }

    #[test]
    fn matching_spec_is_in_sync() {
        let spec = EnsureIpAddressInput {
            address: "10.0.0.1/24".into(),
            interface: "bridge".into(),
            network: Some("10.0.0.0".into()),
            disabled: Some(false),
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_interface_drifts() {
        let spec = EnsureIpAddressInput {
            address: "10.0.0.1/24".into(),
            interface: "ether1".into(),
            ..Default::default()
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }
}
