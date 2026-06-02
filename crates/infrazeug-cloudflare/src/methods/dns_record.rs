//! Ensure a Cloudflare DNS record exists and matches managed fields.

use crate::client::CloudflareClientSource;
use async_trait::async_trait;
use infrazeug_ext_cloudflare_api::dns_record::DnsRecord;
use infrazeug_ext_cloudflare_api::error::CloudflareError;
use infrazeug_ext_cloudflare_api::ListQuery;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};

pub const ENSURE_DNS_RECORD: &str = "cloudflare.ensure_dns_record";

/// Tier-1 method: ensure a DNS record in a zone.
pub type EnsureDnsRecord = EnsureResource<DnsRecordResource>;

/// Construct the registrable [`EnsureDnsRecord`] method for a client source.
pub fn ensure_dns_record(source: CloudflareClientSource) -> EnsureDnsRecord {
    EnsureResource::new(DnsRecordResource::new(source))
}

fn default_record_type() -> String {
    "A".into()
}

/// Desired DNS record. Natural key: zone + `name` + `record_type`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureDnsRecordInput {
    /// Zone id (32-char hex). Provide this or [`zone_name`](Self::zone_name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_id: Option<String>,
    /// Zone DNS name (resolved via `GET /zones?name=…` when `zone_id` is absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_name: Option<String>,
    /// Record name (FQDN or relative to the zone).
    pub name: String,
    /// `A` (default), `AAAA`, `CNAME`, `TXT`, `MX`, …
    #[serde(default = "default_record_type")]
    pub record_type: String,
    /// Record content (IP, hostname, text, …).
    pub content: String,
    /// TTL in seconds (`1` = automatic). Defaults to automatic on create when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
    /// Route through Cloudflare proxy (A/AAAA/CNAME only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxied: Option<bool>,
    /// Priority for MX/SRV records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u16>,
    /// Optional comment (metadata only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// Observed DNS record — managed fields plus the Cloudflare record id.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureDnsRecordOutput {
    pub zone_id: String,
    pub record_id: String,
    pub name: String,
    pub record_type: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxied: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Clone)]
pub struct DnsRecordResource {
    source: CloudflareClientSource,
}

impl DnsRecordResource {
    pub fn new(source: CloudflareClientSource) -> Self {
        Self { source }
    }

    async fn resolve_zone_id(
        &self,
        ctx: &ResourceCtx,
        spec: &EnsureDnsRecordInput,
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

    /// Locate the record this spec manages.
    ///
    /// The apex can hold several records with the same name+type (e.g. a primary
    /// plus a hand-managed secondary MX). Keying on name+type alone picks an
    /// arbitrary one, so a steady-state apply can land on the *other* record,
    /// see drift, and try to reconcile it into a duplicate of the one that
    /// already exists. To avoid that, prefer the record whose content (and
    /// priority, for MX/SRV) matches the spec — the record we actually manage —
    /// and only fall back to the first name+type match when none matches exactly
    /// (the genuine content-drift case, where reconcile then updates it).
    ///
    /// Lists by name only (no `type` filter) to stay robust to query quirks.
    async fn find_managed(
        &self,
        ctx: &ResourceCtx,
        zone_id: &str,
        spec: &EnsureDnsRecordInput,
    ) -> ResourceResult<Option<DnsRecord>> {
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
        Ok(pick_managed(records, spec))
    }

    /// Resolve the live record after Cloudflare rejects a create/update with
    /// "record already exists" — the desired record exists (created out-of-band,
    /// or a write that would duplicate an existing apex record). Adopt it instead
    /// of failing the node, keeping the ensure idempotent.
    async fn adopt_existing(
        &self,
        ctx: &ResourceCtx,
        zone_id: &str,
        spec: &EnsureDnsRecordInput,
    ) -> ResourceResult<EnsureDnsRecordOutput> {
        self.find_managed(ctx, zone_id, spec)
            .await?
            .and_then(|r| to_output(zone_id, r))
            .ok_or_else(|| {
                ResourceError::provider(format!(
                    "cloudflare reports an identical {} record for {:?} already \
                     exists, but it was not returned by the dns_records listing",
                    spec.record_type, spec.name
                ))
            })
    }
}

/// Cloudflare reports that the exact record we asked for already exists: code
/// 81057 ("Record already exists.") / 81058 ("An identical record already
/// exists."), surfaced on both create (POST) and update (PUT). The message is
/// matched unconditionally because the `codes` array is not always populated;
/// note this does NOT match 81053 ("An A, AAAA, or CNAME record with that host
/// already exists"), a conflict with a *different* record that is a real error.
fn is_record_already_exists(err: &CloudflareError) -> bool {
    match err {
        CloudflareError::Api { codes, message, .. } => {
            codes.iter().any(|&c| c == 81057 || c == 81058)
                || message
                    .to_ascii_lowercase()
                    .contains("identical record already exists")
        }
        _ => false,
    }
}

/// Among records sharing the spec's name+type, prefer the one whose content
/// (and priority, when the spec sets it) matches exactly — the record this spec
/// manages — falling back to the first name+type match for the content-drift
/// case. See [`DnsRecordResource::find_managed`].
fn pick_managed(records: Vec<DnsRecord>, spec: &EnsureDnsRecordInput) -> Option<DnsRecord> {
    let mut fallback = None;
    for r in records {
        if r.name != spec.name || r.record_type != spec.record_type {
            continue;
        }
        let content_matches = r.content == spec.content;
        let priority_matches = spec.priority.is_none() || r.priority == spec.priority;
        if content_matches && priority_matches {
            return Some(r);
        }
        fallback.get_or_insert(r);
    }
    fallback
}

fn to_output(zone_id: &str, record: DnsRecord) -> Option<EnsureDnsRecordOutput> {
    let id = record.id?;
    Some(EnsureDnsRecordOutput {
        zone_id: zone_id.to_string(),
        record_id: id,
        name: record.name,
        record_type: record.record_type,
        content: record.content,
        ttl: record.ttl,
        proxied: record.proxied,
        priority: record.priority,
        comment: record.comment,
    })
}

fn build(spec: &EnsureDnsRecordInput) -> DnsRecord {
    DnsRecord {
        name: spec.name.clone(),
        record_type: spec.record_type.clone(),
        content: spec.content.clone(),
        ttl: spec.ttl.or(Some(1)),
        proxied: spec.proxied,
        priority: spec.priority,
        comment: spec.comment.clone(),
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
        let zone_id = self.resolve_zone_id(ctx, spec).await?;
        Ok(self
            .find_managed(ctx, &zone_id, spec)
            .await?
            .and_then(|r| to_output(&zone_id, r)))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let zone_id = self.resolve_zone_id(ctx, spec).await?;
        let client = self.source.client(ctx).await?;
        match client.create_dns_record(&zone_id, &build(spec)).await {
            Ok(created) => to_output(&zone_id, created)
                .ok_or_else(|| ResourceError::provider("created dns record has no id")),
            // The record already exists in Cloudflare — created out-of-band (e.g.
            // the hand-managed apex MX noted in the node table) or by a racing
            // apply between our `observe` and this `create`. Adopt the live record
            // instead of failing the node, so the ensure is idempotent.
            Err(err) if is_record_already_exists(&err) => {
                self.adopt_existing(ctx, &zone_id, spec).await
            }
            Err(err) => Err(ResourceError::provider(err)),
        }
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut diffs = Vec::new();
        if current.content != spec.content {
            diffs.push(format!(
                "content {:?} → {:?}",
                current.content, spec.content
            ));
        }
        if let Some(ttl) = spec.ttl {
            if current.ttl != Some(ttl) {
                diffs.push(format!("ttl {:?} → {}", current.ttl, ttl));
            }
        }
        if let Some(proxied) = spec.proxied {
            if current.proxied != Some(proxied) {
                diffs.push(format!("proxied {:?} → {}", current.proxied, proxied));
            }
        }
        if let Some(priority) = spec.priority {
            if current.priority != Some(priority) {
                diffs.push(format!("priority {:?} → {}", current.priority, priority));
            }
        }
        if let Some(ref comment) = spec.comment {
            if current.comment.as_deref() != Some(comment.as_str()) {
                diffs.push(format!("comment {:?} → {:?}", current.comment, comment));
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
        let zone_id = self.resolve_zone_id(ctx, spec).await?;
        let client = self.source.client(ctx).await?;
        // Fetch exactly the record `observe` identified (by id), so we update the
        // managed record and preserve its unmodelled fields.
        let mut record = client
            .dns_record(&zone_id, &current.record_id)
            .await
            .map_err(ResourceError::provider)?;
        record.content = spec.content.clone();
        if let Some(ttl) = spec.ttl {
            record.ttl = Some(ttl);
        }
        if let Some(proxied) = spec.proxied {
            record.proxied = Some(proxied);
        }
        if let Some(priority) = spec.priority {
            record.priority = Some(priority);
        }
        if spec.comment.is_some() {
            record.comment = spec.comment.clone();
        }
        match client
            .update_dns_record(&zone_id, &current.record_id, &record)
            .await
        {
            Ok(updated) => to_output(&zone_id, updated)
                .ok_or_else(|| ResourceError::provider("updated dns record has no id")),
            // Updating this record to the desired state would duplicate another
            // record that already holds it (e.g. multiple MX rows at the apex).
            // Adopt the existing desired record rather than failing.
            Err(err) if is_record_already_exists(&err) => {
                self.adopt_existing(ctx, &zone_id, spec).await
            }
            Err(err) => Err(ResourceError::provider(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource() -> DnsRecordResource {
        DnsRecordResource::new(CloudflareClientSource::vault("cloud/cloudflare.vault"))
    }

    fn current() -> EnsureDnsRecordOutput {
        EnsureDnsRecordOutput {
            zone_id: "zone123".into(),
            record_id: "rec456".into(),
            name: "www.example.com".into(),
            record_type: "A".into(),
            content: "192.0.2.1".into(),
            ttl: Some(1),
            proxied: Some(true),
            priority: None,
            comment: None,
        }
    }

    #[test]
    fn matching_spec_is_in_sync() {
        let spec = EnsureDnsRecordInput {
            zone_id: Some("zone123".into()),
            name: "www.example.com".into(),
            record_type: "A".into(),
            content: "192.0.2.1".into(),
            proxied: Some(true),
            ..Default::default()
        };
        assert_eq!(resource().diff(&spec, &current()), Drift::InSync);
    }

    #[test]
    fn changed_content_drifts() {
        let spec = EnsureDnsRecordInput {
            zone_id: Some("zone123".into()),
            name: "www.example.com".into(),
            record_type: "A".into(),
            content: "192.0.2.2".into(),
            ..Default::default()
        };
        assert!(matches!(
            resource().diff(&spec, &current()),
            Drift::Drifted(_)
        ));
    }

    fn api_err(codes: Vec<u64>, message: &str) -> CloudflareError {
        CloudflareError::Api {
            status: 400,
            codes,
            message: message.into(),
        }
    }

    #[test]
    fn already_exists_is_adoptable_but_conflicts_are_not() {
        // The error apply hit on cf-dns-mx-apex.
        assert!(is_record_already_exists(&api_err(
            vec![81058],
            "An identical record already exists."
        )));
        assert!(is_record_already_exists(&api_err(
            vec![81057],
            "Record already exists."
        )));
        // Code array sometimes empty; fall back to the message.
        assert!(is_record_already_exists(&api_err(
            vec![],
            "An identical record already exists."
        )));
        // 81053 is a conflict with a *different* record — a real error, must not adopt.
        assert!(!is_record_already_exists(&api_err(
            vec![81053],
            "An A, AAAA, or CNAME record with that host already exists."
        )));
        assert!(!is_record_already_exists(&api_err(
            vec![1003],
            "Invalid zone."
        )));
        assert!(!is_record_already_exists(&CloudflareError::Auth(
            "bad token".into()
        )));
    }

    fn mx(content: &str, priority: u16, id: &str) -> DnsRecord {
        DnsRecord {
            id: Some(id.into()),
            name: "codefionn.eu".into(),
            record_type: "MX".into(),
            content: content.into(),
            priority: Some(priority),
            ..Default::default()
        }
    }

    fn mx_spec() -> EnsureDnsRecordInput {
        EnsureDnsRecordInput {
            zone_name: Some("codefionn.eu".into()),
            name: "codefionn.eu".into(),
            record_type: "MX".into(),
            content: "in1-smtp.messagingengine.com".into(),
            priority: Some(10),
            ..Default::default()
        }
    }

    #[test]
    fn picks_the_spec_matching_mx_not_the_first() {
        // The apex MX bug: the hand-managed secondary is listed first. Keying on
        // name+type alone picked it, drifted, and reconcile duplicated the primary.
        let secondary = mx("in2-smtp.messagingengine.com", 20, "secondary");
        let primary = mx("in1-smtp.messagingengine.com", 10, "primary");
        let picked = pick_managed(vec![secondary, primary], &mx_spec());
        assert_eq!(picked.unwrap().id.as_deref(), Some("primary"));
    }

    #[test]
    fn falls_back_to_first_name_type_match_on_content_drift() {
        // No exact match (content drifted) → return a name+type match so `diff`
        // reports drift and `reconcile` updates it (single-record types: A/AAAA/…).
        let a = DnsRecord {
            id: Some("a1".into()),
            name: "codefionn.eu".into(),
            record_type: "A".into(),
            content: "192.0.2.9".into(), // live, differs from spec
            ..Default::default()
        };
        let spec = EnsureDnsRecordInput {
            name: "codefionn.eu".into(),
            record_type: "A".into(),
            content: "203.0.113.1".into(), // desired
            ..Default::default()
        };
        assert_eq!(
            pick_managed(vec![a], &spec).unwrap().id.as_deref(),
            Some("a1")
        );
    }

    #[test]
    fn ignores_other_names_and_types() {
        let other_type = mx("in1-smtp.messagingengine.com", 10, "txt").clone();
        let mut txt = other_type;
        txt.record_type = "TXT".into();
        assert!(pick_managed(vec![txt], &mx_spec()).is_none());
    }
}
