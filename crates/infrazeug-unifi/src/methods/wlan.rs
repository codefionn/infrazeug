//! Ensure a UniFi wireless network (SSID) exists and matches a small set of
//! managed fields.
//!
//! The pre-shared key is written on create and on any reconcile, but it is **not**
//! captured in the node output and is **not** compared for drift (the resource never
//! reads a secret back into a capture). A passphrase-only rotation will therefore not
//! be detected as drift on its own; change another managed field, or recreate, to
//! force the write.

use crate::client::UnifiClientSource;
use async_trait::async_trait;
use infrazeug_ext_unifi_api::wlan::WlanConf;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_WLAN: &str = "unifi.ensure_wlan";

/// Tier-1 method: ensure a UniFi wireless network.
pub type EnsureWlan = EnsureResource<WlanResource>;

/// Construct the registrable [`EnsureWlan`] method for a client source.
pub fn ensure_wlan(source: UnifiClientSource) -> EnsureWlan {
    EnsureResource::new(WlanResource::new(source))
}

fn default_security() -> String {
    "wpapsk".into()
}

/// Desired SSID — the SSID name is the natural key.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureWlanInput {
    pub name: String,
    /// `wpapsk` (default), `open`, or `wpaeap`.
    #[serde(default = "default_security")]
    pub security: String,
    /// Pre-shared key (for `wpapsk`). Prefer a vault-backed value in the playbook.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
    /// Network this SSID is bridged onto (typically a VLAN network's id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub networkconf_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_guest: Option<bool>,
    /// Defaults to `true` on create when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// User group id (required by some controllers on create).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usergroup_id: Option<String>,
    /// WLAN group id (legacy controllers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wlangroup_id: Option<String>,
    /// AP group ids this SSID broadcasts on (UniFi OS).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ap_group_ids: Option<Vec<String>>,
}

/// Observed SSID — managed fields plus the controller id. The passphrase is never
/// captured.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureWlanOutput {
    pub wlan_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub networkconf_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_guest: Option<bool>,
}

/// A UniFi wireless network as an acquirable resource.
#[derive(Clone)]
pub struct WlanResource {
    source: UnifiClientSource,
}

impl WlanResource {
    pub fn new(source: UnifiClientSource) -> Self {
        Self { source }
    }

    async fn find(&self, ctx: &ResourceCtx, name: &str) -> ResourceResult<Option<WlanConf>> {
        let client = self.source.client(ctx).await?;
        let wlans = client.wlans().await.map_err(ResourceError::provider)?;
        Ok(wlans.into_iter().find(|w| w.name == name))
    }
}

fn to_output(conf: WlanConf) -> Option<EnsureWlanOutput> {
    let id = conf.id?;
    Some(EnsureWlanOutput {
        wlan_id: id,
        name: conf.name,
        security: conf.security,
        enabled: conf.enabled,
        networkconf_id: conf.networkconf_id,
        is_guest: conf.is_guest,
    })
}

#[async_trait]
impl Resource for WlanResource {
    type Spec = EnsureWlanInput;
    type State = EnsureWlanOutput;

    fn kind(&self) -> &'static str {
        ENSURE_WLAN
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
        let body = WlanConf {
            name: spec.name.clone(),
            enabled: Some(spec.enabled.unwrap_or(true)),
            security: Some(spec.security.clone()),
            x_passphrase: spec.passphrase.clone(),
            is_guest: spec.is_guest,
            networkconf_id: spec.networkconf_id.clone(),
            usergroup_id: spec.usergroup_id.clone(),
            wlangroup_id: spec.wlangroup_id.clone(),
            ap_group_ids: spec.ap_group_ids.clone(),
            ..Default::default()
        };
        let created = client
            .create_wlan(&body)
            .await
            .map_err(ResourceError::provider)?;
        to_output(created).ok_or_else(|| ResourceError::provider("created wlan has no id"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if current.security.as_deref() != Some(spec.security.as_str()) {
            diffs.push(format!(
                "security {:?} → {:?}",
                current.security, spec.security
            ));
        }
        if let Some(enabled) = spec.enabled {
            if current.enabled != Some(enabled) {
                diffs.push(format!("enabled {:?} → {}", current.enabled, enabled));
            }
        }
        if let Some(is_guest) = spec.is_guest {
            if current.is_guest != Some(is_guest) {
                diffs.push(format!("is_guest {:?} → {}", current.is_guest, is_guest));
            }
        }
        if let Some(networkconf_id) = &spec.networkconf_id {
            if current.networkconf_id.as_deref() != Some(networkconf_id.as_str()) {
                diffs.push(format!(
                    "networkconf_id {:?} → {:?}",
                    current.networkconf_id, networkconf_id
                ));
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
            .ok_or_else(|| ResourceError::provider("wlan disappeared before reconcile"))?;
        conf.security = Some(spec.security.clone());
        if spec.passphrase.is_some() {
            conf.x_passphrase = spec.passphrase.clone();
        }
        if spec.is_guest.is_some() {
            conf.is_guest = spec.is_guest;
        }
        if spec.networkconf_id.is_some() {
            conf.networkconf_id = spec.networkconf_id.clone();
        }
        if let Some(enabled) = spec.enabled {
            conf.enabled = Some(enabled);
        }
        let updated = client
            .update_wlan(&current.wlan_id, &conf)
            .await
            .map_err(ResourceError::provider)?;
        to_output(updated).ok_or_else(|| ResourceError::provider("updated wlan has no id"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> WlanResource {
        WlanResource::new(UnifiClientSource::vault(
            "https://unifi.local",
            "unifi.vault",
        ))
    }

    fn current() -> EnsureWlanOutput {
        EnsureWlanOutput {
            wlan_id: "wlan-1".into(),
            name: "guest".into(),
            security: Some("wpapsk".into()),
            enabled: Some(true),
            networkconf_id: Some("net-1".into()),
            is_guest: Some(true),
        }
    }

    #[test]
    fn passphrase_only_change_is_not_drift() {
        let spec = EnsureWlanInput {
            name: "guest".into(),
            security: "wpapsk".into(),
            passphrase: Some("rotated".into()),
            networkconf_id: Some("net-1".into()),
            is_guest: Some(true),
            enabled: Some(true),
            ..Default::default()
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_network_binding_drifts() {
        let spec = EnsureWlanInput {
            name: "guest".into(),
            security: "wpapsk".into(),
            networkconf_id: Some("net-2".into()),
            ..Default::default()
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }
}
