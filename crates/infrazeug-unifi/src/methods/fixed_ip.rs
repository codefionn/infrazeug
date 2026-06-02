//! Ensure a known client has a fixed-IP (DHCP) reservation and the desired
//! name / group binding. Keyed by MAC address (`/rest/user`).

use crate::client::UnifiClientSource;
use async_trait::async_trait;
use infrazeug_ext_unifi_api::users::User;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_FIXED_IP: &str = "unifi.ensure_fixed_ip";

/// Tier-1 method: ensure a client's fixed-IP reservation.
pub type EnsureFixedIp = EnsureResource<FixedIpResource>;

/// Construct the registrable [`EnsureFixedIp`] method for a client source.
pub fn ensure_fixed_ip(source: UnifiClientSource) -> EnsureFixedIp {
    EnsureResource::new(FixedIpResource::new(source))
}

/// Desired client reservation. The MAC address is the natural key.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureFixedIpInput {
    /// Client MAC address.
    pub mac: String,
    /// Reserved IP address.
    pub fixed_ip: String,
    /// Network the reservation belongs to (the network's controller id).
    pub network_id: String,
    /// Friendly name shown in the UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// User group (bandwidth profile) to assign.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usergroup_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Observed client reservation — managed fields plus the controller id.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureFixedIpOutput {
    pub user_id: String,
    pub mac: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_fixedip: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_id: Option<String>,
}

/// A known-client fixed-IP reservation as an acquirable resource.
#[derive(Clone)]
pub struct FixedIpResource {
    source: UnifiClientSource,
}

impl FixedIpResource {
    pub fn new(source: UnifiClientSource) -> Self {
        Self { source }
    }

    async fn find(&self, ctx: &ResourceCtx, mac: &str) -> ResourceResult<Option<User>> {
        let client = self.source.client(ctx).await?;
        let users = client.users().await.map_err(ResourceError::provider)?;
        let mac = mac.to_ascii_lowercase();
        Ok(users.into_iter().find(|u| u.mac.eq_ignore_ascii_case(&mac)))
    }
}

fn to_output(user: User) -> Option<EnsureFixedIpOutput> {
    let id = user.id?;
    Some(EnsureFixedIpOutput {
        user_id: id,
        mac: user.mac,
        name: user.name,
        fixed_ip: user.fixed_ip,
        use_fixedip: user.use_fixedip,
        network_id: user.network_id,
    })
}

fn overlay(user: &mut User, spec: &EnsureFixedIpInput) {
    user.mac = spec.mac.to_ascii_lowercase();
    user.fixed_ip = Some(spec.fixed_ip.clone());
    user.use_fixedip = Some(true);
    user.network_id = Some(spec.network_id.clone());
    if spec.name.is_some() {
        user.name = spec.name.clone();
    }
    if spec.usergroup_id.is_some() {
        user.usergroup_id = spec.usergroup_id.clone();
    }
    if spec.note.is_some() {
        user.note = spec.note.clone();
    }
}

#[async_trait]
impl Resource for FixedIpResource {
    type Spec = EnsureFixedIpInput;
    type State = EnsureFixedIpOutput;

    fn kind(&self) -> &'static str {
        ENSURE_FIXED_IP
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        Ok(self.find(ctx, &spec.mac).await?.and_then(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let mut user = User::default();
        overlay(&mut user, spec);
        let created = client
            .create_user(&user)
            .await
            .map_err(ResourceError::provider)?;
        to_output(created).ok_or_else(|| ResourceError::provider("created client has no id"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if current.use_fixedip != Some(true) {
            diffs.push(format!("use_fixedip {:?} → true", current.use_fixedip));
        }
        if current.fixed_ip.as_deref() != Some(spec.fixed_ip.as_str()) {
            diffs.push(format!(
                "fixed_ip {:?} → {:?}",
                current.fixed_ip, spec.fixed_ip
            ));
        }
        if current.network_id.as_deref() != Some(spec.network_id.as_str()) {
            diffs.push(format!(
                "network_id {:?} → {:?}",
                current.network_id, spec.network_id
            ));
        }
        if let Some(name) = &spec.name {
            if current.name.as_deref() != Some(name.as_str()) {
                diffs.push(format!("name {:?} → {:?}", current.name, name));
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
        let mut user = self
            .find(ctx, &spec.mac)
            .await?
            .ok_or_else(|| ResourceError::provider("client disappeared before reconcile"))?;
        overlay(&mut user, spec);
        let updated = client
            .update_user(&current.user_id, &user)
            .await
            .map_err(ResourceError::provider)?;
        to_output(updated).ok_or_else(|| ResourceError::provider("updated client has no id"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> FixedIpResource {
        FixedIpResource::new(UnifiClientSource::vault(
            "https://unifi.local",
            "unifi.vault",
        ))
    }

    fn spec() -> EnsureFixedIpInput {
        EnsureFixedIpInput {
            mac: "AA:BB:CC:DD:EE:FF".into(),
            fixed_ip: "10.0.0.50".into(),
            network_id: "net-1".into(),
            name: Some("printer".into()),
            ..Default::default()
        }
    }

    fn current() -> EnsureFixedIpOutput {
        EnsureFixedIpOutput {
            user_id: "u-1".into(),
            mac: "aa:bb:cc:dd:ee:ff".into(),
            name: Some("printer".into()),
            fixed_ip: Some("10.0.0.50".into()),
            use_fixedip: Some(true),
            network_id: Some("net-1".into()),
        }
    }

    #[test]
    fn matching_reservation_in_sync() {
        assert_eq!(resource().diff(&spec(), &current()), Drift::InSync);
    }

    #[test]
    fn changed_ip_drifts() {
        let mut s = spec();
        s.fixed_ip = "10.0.0.51".into();
        assert!(matches!(resource().diff(&s, &current()), Drift::Drifted(_)));
    }
}
