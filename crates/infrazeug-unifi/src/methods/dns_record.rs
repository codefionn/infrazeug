//! Ensure a controller-served local DNS record exists and matches its value.
//!
//! The natural key is `(name, record_type)`: this manages a single record per name
//! and type, reconciling its value/TTL on drift. For round-robin sets of several
//! records sharing a name+type, manage each with a distinct node and rely on the
//! controller id (not modelled here) — or use the ext client directly.

use crate::client::UnifiClientSource;
use async_trait::async_trait;
use infrazeug_ext_unifi_api::dns::DnsRecord;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_DNS_RECORD: &str = "unifi.ensure_dns_record";

/// Tier-1 method: ensure a local DNS record.
pub type EnsureDnsRecord = EnsureResource<DnsRecordResource>;

/// Construct the registrable [`EnsureDnsRecord`] method for a client source.
pub fn ensure_dns_record(source: UnifiClientSource) -> EnsureDnsRecord {
    EnsureResource::new(DnsRecordResource::new(source))
}

fn default_record_type() -> String {
    "A".into()
}

/// Desired DNS record. `name` + `record_type` form the natural key.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureDnsRecordInput {
    /// Hostname being resolved (e.g. `nas.lan`).
    pub name: String,
    /// `A` (default), `AAAA`, `CNAME`, `TXT`, `MX`, `SRV`, `NS`, `PTR`.
    #[serde(default = "default_record_type")]
    pub record_type: String,
    /// Record value (target IP, hostname, text, …).
    pub value: String,
    /// Defaults to `true` on create when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
    /// Priority (for `MX` / `SRV`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
    /// Port (for `SRV`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u32>,
    /// Weight (for `SRV`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<u32>,
}

/// Observed DNS record — managed fields plus the controller id.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureDnsRecordOutput {
    pub record_id: String,
    pub name: String,
    pub record_type: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
}

/// A local DNS record as an acquirable resource.
#[derive(Clone)]
pub struct DnsRecordResource {
    source: UnifiClientSource,
}

impl DnsRecordResource {
    pub fn new(source: UnifiClientSource) -> Self {
        Self { source }
    }

    async fn find(
        &self,
        ctx: &ResourceCtx,
        name: &str,
        record_type: &str,
    ) -> ResourceResult<Option<DnsRecord>> {
        let client = self.source.client(ctx).await?;
        let records = client
            .dns_records()
            .await
            .map_err(ResourceError::provider)?;
        Ok(records
            .into_iter()
            .find(|r| r.key == name && r.record_type == record_type))
    }
}

fn to_output(record: DnsRecord) -> Option<EnsureDnsRecordOutput> {
    let id = record.id?;
    Some(EnsureDnsRecordOutput {
        record_id: id,
        name: record.key,
        record_type: record.record_type,
        value: record.value,
        enabled: record.enabled,
        ttl: record.ttl,
    })
}

fn build(spec: &EnsureDnsRecordInput) -> DnsRecord {
    DnsRecord {
        key: spec.name.clone(),
        record_type: spec.record_type.clone(),
        value: spec.value.clone(),
        enabled: Some(spec.enabled.unwrap_or(true)),
        ttl: spec.ttl,
        priority: spec.priority,
        port: spec.port,
        weight: spec.weight,
        ..Default::default()
    }
}

#[async_trait]
impl Resource for DnsRecordResource {
    type Spec = EnsureDnsRecordInput;
    type State = EnsureDnsRecordOutput;

    fn kind(&self) -> &'static str {
        ENSURE_DNS_RECORD
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        Ok(self
            .find(ctx, &spec.name, &spec.record_type)
            .await?
            .and_then(to_output))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let created = client
            .create_dns_record(&build(spec))
            .await
            .map_err(ResourceError::provider)?;
        to_output(created).ok_or_else(|| ResourceError::provider("created dns record has no id"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if current.value != spec.value {
            diffs.push(format!("value {:?} → {:?}", current.value, spec.value));
        }
        if let Some(enabled) = spec.enabled {
            if current.enabled != Some(enabled) {
                diffs.push(format!("enabled {:?} → {}", current.enabled, enabled));
            }
        }
        if let Some(ttl) = spec.ttl {
            if current.ttl != Some(ttl) {
                diffs.push(format!("ttl {:?} → {}", current.ttl, ttl));
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
        let mut record = self
            .find(ctx, &spec.name, &spec.record_type)
            .await?
            .ok_or_else(|| ResourceError::provider("dns record disappeared before reconcile"))?;
        record.value = spec.value.clone();
        if let Some(enabled) = spec.enabled {
            record.enabled = Some(enabled);
        }
        if spec.ttl.is_some() {
            record.ttl = spec.ttl;
        }
        if spec.priority.is_some() {
            record.priority = spec.priority;
        }
        if spec.port.is_some() {
            record.port = spec.port;
        }
        if spec.weight.is_some() {
            record.weight = spec.weight;
        }
        let updated = client
            .update_dns_record(&current.record_id, &record)
            .await
            .map_err(ResourceError::provider)?;
        to_output(updated).ok_or_else(|| ResourceError::provider("updated dns record has no id"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> DnsRecordResource {
        DnsRecordResource::new(UnifiClientSource::vault(
            "https://unifi.local",
            "unifi.vault",
        ))
    }

    fn current() -> EnsureDnsRecordOutput {
        EnsureDnsRecordOutput {
            record_id: "dns-1".into(),
            name: "nas.lan".into(),
            record_type: "A".into(),
            value: "10.0.0.5".into(),
            enabled: Some(true),
            ttl: None,
        }
    }

    #[test]
    fn matching_value_is_in_sync() {
        let spec = EnsureDnsRecordInput {
            name: "nas.lan".into(),
            record_type: "A".into(),
            value: "10.0.0.5".into(),
            ..Default::default()
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_value_drifts() {
        let spec = EnsureDnsRecordInput {
            name: "nas.lan".into(),
            record_type: "A".into(),
            value: "10.0.0.9".into(),
            ..Default::default()
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }
}
