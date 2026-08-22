//! Ensure every DNS record at a name has a given type is absent.

use crate::client::CloudflareClientSource;
use async_trait::async_trait;
use infrazeug_ext_cloudflare_api::dns_record::DnsRecord;
use infrazeug_ext_cloudflare_api::ListQuery;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_DNS_RECORD_ABSENT: &str = "cloudflare.ensure_dns_record_absent";

/// Tier-1 method: remove every record with this exact DNS name and type.
pub type EnsureDnsRecordAbsent = EnsureResource<DnsRecordAbsentResource>;

/// Construct the registrable [`EnsureDnsRecordAbsent`] method for a client source.
pub fn ensure_dns_record_absent(source: CloudflareClientSource) -> EnsureDnsRecordAbsent {
    EnsureResource::new(DnsRecordAbsentResource::new(source))
}

fn default_record_type() -> String {
    "A".into()
}

/// DNS records to remove. Matching is deliberately exact on both FQDN and type,
/// so unrelated records at the same name remain untouched.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureDnsRecordAbsentInput {
    /// Zone id (32-char hex). Provide this or [`zone_name`](Self::zone_name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_id: Option<String>,
    /// Zone DNS name, resolved when `zone_id` is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_name: Option<String>,
    /// Exact DNS record name to remove.
    pub name: String,
    /// Exact DNS record type to remove.
    #[serde(default = "default_record_type")]
    pub record_type: String,
}

/// A successful absence check. `EnsureResource` models the desired absence as
/// the resource state: `observe` returns this only once no matching rows remain.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureDnsRecordAbsentOutput {
    pub zone_id: String,
    pub name: String,
    pub record_type: String,
}

#[derive(Clone)]
pub struct DnsRecordAbsentResource {
    source: CloudflareClientSource,
}

impl DnsRecordAbsentResource {
    pub fn new(source: CloudflareClientSource) -> Self {
        Self { source }
    }

    async fn resolve_zone_id(
        &self,
        ctx: &ResourceCtx,
        spec: &EnsureDnsRecordAbsentInput,
    ) -> ResourceResult<String> {
        if let Some(id) = &spec.zone_id {
            return Ok(id.clone());
        }
        let name = spec
            .zone_name
            .as_deref()
            .ok_or_else(|| ResourceError::provider("zone_id or zone_name is required"))?;
        let client = self.source.client(ctx).await?;
        client
            .zone_id_by_name(name)
            .await
            .map_err(ResourceError::provider)
    }

    async fn matching_records(
        &self,
        ctx: &ResourceCtx,
        zone_id: &str,
        spec: &EnsureDnsRecordAbsentInput,
    ) -> ResourceResult<Vec<DnsRecord>> {
        let client = self.source.client(ctx).await?;
        let query = ListQuery {
            name: Some(spec.name.clone()),
            per_page: Some(100),
            ..Default::default()
        };
        let records = client
            .dns_records(zone_id, &query)
            .await
            .map_err(ResourceError::provider)?;
        Ok(matching_records(records, spec))
    }
}

fn matching_records(records: Vec<DnsRecord>, spec: &EnsureDnsRecordAbsentInput) -> Vec<DnsRecord> {
    records
        .into_iter()
        .filter(|record| record.name == spec.name && record.record_type == spec.record_type)
        .collect()
}

#[async_trait]
impl Resource for DnsRecordAbsentResource {
    type Spec = EnsureDnsRecordAbsentInput;
    type State = EnsureDnsRecordAbsentOutput;

    fn kind(&self) -> &'static str {
        ENSURE_DNS_RECORD_ABSENT
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let zone_id = self.resolve_zone_id(ctx, spec).await?;
        if self.matching_records(ctx, &zone_id, spec).await?.is_empty() {
            Ok(Some(EnsureDnsRecordAbsentOutput {
                zone_id,
                name: spec.name.clone(),
                record_type: spec.record_type.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let zone_id = self.resolve_zone_id(ctx, spec).await?;
        let records = self.matching_records(ctx, &zone_id, spec).await?;
        let client = self.source.client(ctx).await?;
        for record in records {
            let id = record.id.ok_or_else(|| {
                ResourceError::provider(format!(
                    "Cloudflare returned {} record {:?} without an id",
                    spec.record_type, spec.name
                ))
            })?;
            client
                .delete_dns_record(&zone_id, &id)
                .await
                .map_err(ResourceError::provider)?;
        }
        Ok(EnsureDnsRecordAbsentOutput {
            zone_id,
            name: spec.name.clone(),
            record_type: spec.record_type.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_selects_the_exact_name_and_type() {
        let spec = EnsureDnsRecordAbsentInput {
            name: "*.example.com".into(),
            record_type: "A".into(),
            ..Default::default()
        };
        let records = vec![
            DnsRecord {
                id: Some("a".into()),
                name: "*.example.com".into(),
                record_type: "A".into(),
                content: "192.0.2.1".into(),
                ..Default::default()
            },
            DnsRecord {
                id: Some("aaaa".into()),
                name: "*.example.com".into(),
                record_type: "AAAA".into(),
                content: "2001:db8::1".into(),
                ..Default::default()
            },
            DnsRecord {
                id: Some("other".into()),
                name: "www.example.com".into(),
                record_type: "A".into(),
                content: "192.0.2.1".into(),
                ..Default::default()
            },
        ];

        let matched = matching_records(records, &spec);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].id.as_deref(), Some("a"));
    }
}
