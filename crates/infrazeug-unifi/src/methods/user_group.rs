//! Ensure a user group (client bandwidth profile) exists and matches its limits.

use crate::client::UnifiClientSource;
use async_trait::async_trait;
use infrazeug_ext_unifi_api::user_group::UserGroup;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_USER_GROUP: &str = "unifi.ensure_user_group";

/// Tier-1 method: ensure a user group.
pub type EnsureUserGroup = EnsureResource<UserGroupResource>;

/// Construct the registrable [`EnsureUserGroup`] method for a client source.
pub fn ensure_user_group(source: UnifiClientSource) -> EnsureUserGroup {
    EnsureResource::new(UserGroupResource::new(source))
}

fn default_unlimited() -> i32 {
    -1
}

/// Desired user group. The name is the natural key. Rates are kbps; `-1` =
/// unlimited (the default).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EnsureUserGroupInput {
    pub name: String,
    #[serde(default = "default_unlimited")]
    pub qos_rate_max_down: i32,
    #[serde(default = "default_unlimited")]
    pub qos_rate_max_up: i32,
}

impl Default for EnsureUserGroupInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            qos_rate_max_down: default_unlimited(),
            qos_rate_max_up: default_unlimited(),
        }
    }
}

/// Observed user group — managed fields plus the controller id.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureUserGroupOutput {
    pub group_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qos_rate_max_down: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qos_rate_max_up: Option<i32>,
}

/// A user group as an acquirable resource.
#[derive(Clone)]
pub struct UserGroupResource {
    source: UnifiClientSource,
}

impl UserGroupResource {
    pub fn new(source: UnifiClientSource) -> Self {
        Self { source }
    }

    async fn find(&self, ctx: &ResourceCtx, name: &str) -> ResourceResult<Option<UserGroup>> {
        let client = self.source.client(ctx).await?;
        let groups = client
            .user_groups()
            .await
            .map_err(ResourceError::provider)?;
        Ok(groups.into_iter().find(|g| g.name == name))
    }
}

fn to_output(group: UserGroup) -> Option<EnsureUserGroupOutput> {
    let id = group.id?;
    Some(EnsureUserGroupOutput {
        group_id: id,
        name: group.name,
        qos_rate_max_down: group.qos_rate_max_down,
        qos_rate_max_up: group.qos_rate_max_up,
    })
}

#[async_trait]
impl Resource for UserGroupResource {
    type Spec = EnsureUserGroupInput;
    type State = EnsureUserGroupOutput;

    fn kind(&self) -> &'static str {
        ENSURE_USER_GROUP
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
        let body = UserGroup {
            name: spec.name.clone(),
            qos_rate_max_down: Some(spec.qos_rate_max_down),
            qos_rate_max_up: Some(spec.qos_rate_max_up),
            ..Default::default()
        };
        let created = client
            .create_user_group(&body)
            .await
            .map_err(ResourceError::provider)?;
        to_output(created).ok_or_else(|| ResourceError::provider("created user group has no id"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if current.qos_rate_max_down != Some(spec.qos_rate_max_down) {
            diffs.push(format!(
                "qos_rate_max_down {:?} → {}",
                current.qos_rate_max_down, spec.qos_rate_max_down
            ));
        }
        if current.qos_rate_max_up != Some(spec.qos_rate_max_up) {
            diffs.push(format!(
                "qos_rate_max_up {:?} → {}",
                current.qos_rate_max_up, spec.qos_rate_max_up
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
        let mut group = self
            .find(ctx, &spec.name)
            .await?
            .ok_or_else(|| ResourceError::provider("user group disappeared before reconcile"))?;
        group.qos_rate_max_down = Some(spec.qos_rate_max_down);
        group.qos_rate_max_up = Some(spec.qos_rate_max_up);
        let updated = client
            .update_user_group(&current.group_id, &group)
            .await
            .map_err(ResourceError::provider)?;
        to_output(updated).ok_or_else(|| ResourceError::provider("updated user group has no id"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> UserGroupResource {
        UserGroupResource::new(UnifiClientSource::vault(
            "https://unifi.local",
            "unifi.vault",
        ))
    }

    fn current() -> EnsureUserGroupOutput {
        EnsureUserGroupOutput {
            group_id: "ug-1".into(),
            name: "throttled".into(),
            qos_rate_max_down: Some(50_000),
            qos_rate_max_up: Some(10_000),
        }
    }

    #[test]
    fn matching_limits_in_sync() {
        let spec = EnsureUserGroupInput {
            name: "throttled".into(),
            qos_rate_max_down: 50_000,
            qos_rate_max_up: 10_000,
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_limit_drifts() {
        let spec = EnsureUserGroupInput {
            name: "throttled".into(),
            qos_rate_max_down: 25_000,
            qos_rate_max_up: 10_000,
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }
}
