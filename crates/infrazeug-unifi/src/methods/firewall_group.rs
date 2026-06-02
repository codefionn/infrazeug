//! Ensure a firewall group (address / port set) exists and matches its members.

use crate::client::UnifiClientSource;
use async_trait::async_trait;
use infrazeug_ext_unifi_api::firewall_group::FirewallGroup;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_FIREWALL_GROUP: &str = "unifi.ensure_firewall_group";

/// Tier-1 method: ensure a firewall group.
pub type EnsureFirewallGroup = EnsureResource<FirewallGroupResource>;

/// Construct the registrable [`EnsureFirewallGroup`] method for a client source.
pub fn ensure_firewall_group(source: UnifiClientSource) -> EnsureFirewallGroup {
    EnsureResource::new(FirewallGroupResource::new(source))
}

fn default_group_type() -> String {
    "address-group".into()
}

/// Desired firewall group. The name is the natural key.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureFirewallGroupInput {
    pub name: String,
    /// `address-group` (default), `port-group`, or `ipv6-address-group`.
    #[serde(default = "default_group_type")]
    pub group_type: String,
    /// Members — IPs/CIDRs for address groups, ports for port groups.
    #[serde(default)]
    pub members: Vec<String>,
}

/// Observed firewall group — managed fields plus the controller id.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureFirewallGroupOutput {
    pub group_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_type: Option<String>,
    #[serde(default)]
    pub members: Vec<String>,
}

/// A firewall group as an acquirable resource.
#[derive(Clone)]
pub struct FirewallGroupResource {
    source: UnifiClientSource,
}

impl FirewallGroupResource {
    pub fn new(source: UnifiClientSource) -> Self {
        Self { source }
    }

    async fn find(&self, ctx: &ResourceCtx, name: &str) -> ResourceResult<Option<FirewallGroup>> {
        let client = self.source.client(ctx).await?;
        let groups = client
            .firewall_groups()
            .await
            .map_err(ResourceError::provider)?;
        Ok(groups.into_iter().find(|g| g.name == name))
    }
}

fn to_output(group: FirewallGroup) -> Option<EnsureFirewallGroupOutput> {
    let id = group.id?;
    Some(EnsureFirewallGroupOutput {
        group_id: id,
        name: group.name,
        group_type: group.group_type,
        members: group.group_members.unwrap_or_default(),
    })
}

#[async_trait]
impl Resource for FirewallGroupResource {
    type Spec = EnsureFirewallGroupInput;
    type State = EnsureFirewallGroupOutput;

    fn kind(&self) -> &'static str {
        ENSURE_FIREWALL_GROUP
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
        let body = FirewallGroup {
            name: spec.name.clone(),
            group_type: Some(spec.group_type.clone()),
            group_members: Some(spec.members.clone()),
            ..Default::default()
        };
        let created = client
            .create_firewall_group(&body)
            .await
            .map_err(ResourceError::provider)?;
        to_output(created)
            .ok_or_else(|| ResourceError::provider("created firewall group has no id"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if current.group_type.as_deref() != Some(spec.group_type.as_str()) {
            diffs.push(format!(
                "group_type {:?} → {:?}",
                current.group_type, spec.group_type
            ));
        }
        if current.members != spec.members {
            diffs.push(format!(
                "members {:?} → {:?}",
                current.members, spec.members
            ));
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
        let mut group = self.find(ctx, &spec.name).await?.ok_or_else(|| {
            ResourceError::provider("firewall group disappeared before reconcile")
        })?;
        group.group_type = Some(spec.group_type.clone());
        group.group_members = Some(spec.members.clone());
        let updated = client
            .update_firewall_group(&current.group_id, &group)
            .await
            .map_err(ResourceError::provider)?;
        to_output(updated)
            .ok_or_else(|| ResourceError::provider("updated firewall group has no id"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> FirewallGroupResource {
        FirewallGroupResource::new(UnifiClientSource::vault(
            "https://unifi.local",
            "unifi.vault",
        ))
    }

    fn current() -> EnsureFirewallGroupOutput {
        EnsureFirewallGroupOutput {
            group_id: "fg-1".into(),
            name: "admins".into(),
            group_type: Some("address-group".into()),
            members: vec!["10.0.0.2".into(), "10.0.0.3".into()],
        }
    }

    #[test]
    fn matching_members_in_sync() {
        let spec = EnsureFirewallGroupInput {
            name: "admins".into(),
            group_type: "address-group".into(),
            members: vec!["10.0.0.2".into(), "10.0.0.3".into()],
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_members_drift() {
        let spec = EnsureFirewallGroupInput {
            name: "admins".into(),
            group_type: "address-group".into(),
            members: vec!["10.0.0.2".into()],
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }
}
