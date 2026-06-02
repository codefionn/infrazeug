//! Ensure a UniFi port-forward (destination-NAT) rule exists and matches its
//! managed fields.

use crate::client::UnifiClientSource;
use async_trait::async_trait;
use infrazeug_ext_unifi_api::port_forward::PortForward;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_PORT_FORWARD: &str = "unifi.ensure_port_forward";

/// Tier-1 method: ensure a UniFi port-forward rule.
pub type EnsurePortForward = EnsureResource<PortForwardResource>;

/// Construct the registrable [`EnsurePortForward`] method for a client source.
pub fn ensure_port_forward(source: UnifiClientSource) -> EnsurePortForward {
    EnsureResource::new(PortForwardResource::new(source))
}

fn default_interface() -> String {
    "wan".into()
}
fn default_proto() -> String {
    "tcp_udp".into()
}
fn default_src() -> String {
    "any".into()
}

/// Desired port-forward rule. The rule name is the natural key. Ports are strings
/// so ranges (`"8000-8010"`) are expressible.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsurePortForwardInput {
    pub name: String,
    /// Internal destination host.
    pub fwd: String,
    /// Internal destination port (or range).
    pub fwd_port: String,
    /// External (WAN) port (or range).
    pub dst_port: String,
    /// `tcp`, `udp`, or `tcp_udp` (default).
    #[serde(default = "default_proto")]
    pub proto: String,
    /// WAN interface (default `wan`).
    #[serde(default = "default_interface")]
    pub pfwd_interface: String,
    /// Source restriction (`any` [default] or a CIDR).
    #[serde(default = "default_src")]
    pub src: String,
    /// Defaults to `true` on create when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Observed port-forward rule — managed fields plus the controller id.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsurePortForwardOutput {
    pub port_forward_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fwd_port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proto: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// A UniFi port-forward rule as an acquirable resource.
#[derive(Clone)]
pub struct PortForwardResource {
    source: UnifiClientSource,
}

impl PortForwardResource {
    pub fn new(source: UnifiClientSource) -> Self {
        Self { source }
    }

    async fn find(&self, ctx: &ResourceCtx, name: &str) -> ResourceResult<Option<PortForward>> {
        let client = self.source.client(ctx).await?;
        let rules = client
            .port_forwards()
            .await
            .map_err(ResourceError::provider)?;
        Ok(rules.into_iter().find(|r| r.name == name))
    }
}

fn to_output(rule: PortForward) -> Option<EnsurePortForwardOutput> {
    let id = rule.id?;
    Some(EnsurePortForwardOutput {
        port_forward_id: id,
        name: rule.name,
        fwd: rule.fwd,
        fwd_port: rule.fwd_port,
        dst_port: rule.dst_port,
        proto: rule.proto,
        enabled: rule.enabled,
    })
}

#[async_trait]
impl Resource for PortForwardResource {
    type Spec = EnsurePortForwardInput;
    type State = EnsurePortForwardOutput;

    fn kind(&self) -> &'static str {
        ENSURE_PORT_FORWARD
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
        let body = build(spec);
        let created = client
            .create_port_forward(&body)
            .await
            .map_err(ResourceError::provider)?;
        to_output(created).ok_or_else(|| ResourceError::provider("created port forward has no id"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if current.fwd.as_deref() != Some(spec.fwd.as_str()) {
            diffs.push(format!("fwd {:?} → {:?}", current.fwd, spec.fwd));
        }
        if current.fwd_port.as_deref() != Some(spec.fwd_port.as_str()) {
            diffs.push(format!(
                "fwd_port {:?} → {:?}",
                current.fwd_port, spec.fwd_port
            ));
        }
        if current.dst_port.as_deref() != Some(spec.dst_port.as_str()) {
            diffs.push(format!(
                "dst_port {:?} → {:?}",
                current.dst_port, spec.dst_port
            ));
        }
        if current.proto.as_deref() != Some(spec.proto.as_str()) {
            diffs.push(format!("proto {:?} → {:?}", current.proto, spec.proto));
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
        // Re-read so unmanaged fields survive the PUT, then overlay managed fields.
        let mut conf = self
            .find(ctx, &spec.name)
            .await?
            .ok_or_else(|| ResourceError::provider("port forward disappeared before reconcile"))?;
        let desired = build(spec);
        conf.fwd = desired.fwd;
        conf.fwd_port = desired.fwd_port;
        conf.dst_port = desired.dst_port;
        conf.proto = desired.proto;
        conf.pfwd_interface = desired.pfwd_interface;
        conf.src = desired.src;
        conf.enabled = desired.enabled.or(conf.enabled);
        let updated = client
            .update_port_forward(&current.port_forward_id, &conf)
            .await
            .map_err(ResourceError::provider)?;
        to_output(updated).ok_or_else(|| ResourceError::provider("updated port forward has no id"))
    }
}

fn build(spec: &EnsurePortForwardInput) -> PortForward {
    PortForward {
        name: spec.name.clone(),
        enabled: Some(spec.enabled.unwrap_or(true)),
        pfwd_interface: Some(spec.pfwd_interface.clone()),
        fwd: Some(spec.fwd.clone()),
        fwd_port: Some(spec.fwd_port.clone()),
        dst_port: Some(spec.dst_port.clone()),
        proto: Some(spec.proto.clone()),
        src: Some(spec.src.clone()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> PortForwardResource {
        PortForwardResource::new(UnifiClientSource::vault(
            "https://unifi.local",
            "unifi.vault",
        ))
    }

    fn spec() -> EnsurePortForwardInput {
        EnsurePortForwardInput {
            name: "web".into(),
            fwd: "10.0.0.10".into(),
            fwd_port: "80".into(),
            dst_port: "80".into(),
            proto: "tcp".into(),
            pfwd_interface: "wan".into(),
            src: "any".into(),
            enabled: Some(true),
        }
    }

    fn current() -> EnsurePortForwardOutput {
        EnsurePortForwardOutput {
            port_forward_id: "pf-1".into(),
            name: "web".into(),
            fwd: Some("10.0.0.10".into()),
            fwd_port: Some("80".into()),
            dst_port: Some("80".into()),
            proto: Some("tcp".into()),
            enabled: Some(true),
        }
    }

    #[test]
    fn matching_spec_is_in_sync() {
        assert_eq!(resource().diff(&spec(), &current()), Drift::InSync);
    }

    #[test]
    fn changed_target_drifts() {
        let mut s = spec();
        s.fwd = "10.0.0.11".into();
        assert!(matches!(resource().diff(&s, &current()), Drift::Drifted(_)));
    }
}
