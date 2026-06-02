//! Ensure a Cloudflare zone setting matches the desired value.

use crate::client::CloudflareClientSource;
use crate::methods::zone::resolve_zone_id;
use async_trait::async_trait;
use infrazeug_ext_cloudflare_api::zone_setting::{ZoneSetting, ZoneSettingUpdate};
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ENSURE_ZONE_SETTING: &str = "cloudflare.ensure_zone_setting";

/// Tier-1 method: ensure a zone setting value.
pub type EnsureZoneSetting = EnsureResource<ZoneSettingResource>;

/// Construct the registrable [`EnsureZoneSetting`] method for a client source.
pub fn ensure_zone_setting(source: CloudflareClientSource) -> EnsureZoneSetting {
    EnsureResource::new(ZoneSettingResource::new(source))
}

/// Desired zone setting. Natural key: zone + `setting_id` (e.g. `ssl`, `always_use_https`).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureZoneSettingInput {
    /// Zone id (32-char hex). Provide this or [`zone_name`](Self::zone_name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_id: Option<String>,
    /// Zone DNS name (resolved via `GET /zones?name=…` when `zone_id` is absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_name: Option<String>,
    /// Setting id, e.g. `ssl`, `always_use_https`, `min_tls_version`.
    pub setting_id: String,
    /// Desired setting value (`"on"`/`"off"`, `"full"`, `"strict"`, …).
    pub value: Value,
}

/// Observed zone setting — managed fields from the API.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureZoneSettingOutput {
    pub zone_id: String,
    pub setting_id: String,
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editable: Option<bool>,
}

#[derive(Clone)]
pub struct ZoneSettingResource {
    source: CloudflareClientSource,
}

impl ZoneSettingResource {
    pub fn new(source: CloudflareClientSource) -> Self {
        Self { source }
    }
}

fn to_output(zone_id: &str, setting: ZoneSetting) -> EnsureZoneSettingOutput {
    EnsureZoneSettingOutput {
        zone_id: zone_id.to_string(),
        setting_id: setting.id,
        value: setting.value,
        editable: setting.editable,
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    a == b
}

#[async_trait]
impl Resource for ZoneSettingResource {
    type Spec = EnsureZoneSettingInput;
    type State = EnsureZoneSettingOutput;

    fn kind(&self) -> &'static str {
        ENSURE_ZONE_SETTING
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let zone_id = resolve_zone_id(&self.source, ctx, &spec.zone_id, &spec.zone_name).await?;
        let client = self.source.client(ctx).await?;
        let setting = client
            .zone_setting(&zone_id, &spec.setting_id)
            .await
            .map_err(ResourceError::provider)?;
        Ok(Some(to_output(&zone_id, setting)))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let zone_id = resolve_zone_id(&self.source, ctx, &spec.zone_id, &spec.zone_name).await?;
        let client = self.source.client(ctx).await?;
        let updated = client
            .update_zone_setting(
                &zone_id,
                &spec.setting_id,
                &ZoneSettingUpdate {
                    value: spec.value.clone(),
                },
            )
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(&zone_id, updated))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        if values_equal(&current.value, &spec.value) {
            Drift::InSync
        } else {
            Drift::Drifted(format!("value {:?} → {:?}", current.value, spec.value))
        }
    }

    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        _current: Self::State,
    ) -> ResourceResult<Self::State> {
        self.create(ctx, spec).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn resource() -> ZoneSettingResource {
        ZoneSettingResource::new(CloudflareClientSource::vault("cloud/cloudflare.vault"))
    }

    #[test]
    fn matching_value_is_in_sync() {
        let spec = EnsureZoneSettingInput {
            zone_id: Some("zone123".into()),
            setting_id: "ssl".into(),
            value: json!("full"),
            ..Default::default()
        };
        let current = EnsureZoneSettingOutput {
            zone_id: "zone123".into(),
            setting_id: "ssl".into(),
            value: json!("full"),
            editable: Some(true),
        };
        assert_eq!(resource().diff(&spec, &current), Drift::InSync);
    }

    #[test]
    fn changed_value_drifts() {
        let spec = EnsureZoneSettingInput {
            zone_id: Some("zone123".into()),
            setting_id: "always_use_https".into(),
            value: json!("on"),
            ..Default::default()
        };
        let current = EnsureZoneSettingOutput {
            zone_id: "zone123".into(),
            setting_id: "always_use_https".into(),
            value: json!("off"),
            editable: Some(true),
        };
        assert!(matches!(
            resource().diff(&spec, &current),
            Drift::Drifted(_)
        ));
    }
}
