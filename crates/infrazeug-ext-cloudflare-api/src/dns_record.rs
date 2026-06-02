//! DNS records (`/zones/{zone_id}/dns_records`).

use crate::client::CloudflareClient;
use crate::error::Result;
use crate::types::ListQuery;
use serde::{Deserialize, Serialize};

/// A DNS record in a zone.
///
/// Unmodelled API fields round-trip through [`extra`](Self::extra).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DnsRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_name: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl CloudflareClient {
    /// `GET /zones/{zone_id}/dns_records` — list DNS records (all pages).
    pub async fn dns_records(&self, zone_id: &str, query: &ListQuery) -> Result<Vec<DnsRecord>> {
        let path = format!("/zones/{}/dns_records", self.encode_path(zone_id));
        self.get_all(&path, query.clone()).await
    }

    /// `GET /zones/{zone_id}/dns_records/{id}` — fetch one record.
    pub async fn dns_record(&self, zone_id: &str, record_id: &str) -> Result<DnsRecord> {
        let path = format!(
            "/zones/{}/dns_records/{}",
            self.encode_path(zone_id),
            self.encode_path(record_id)
        );
        let (record, _) = self.get(&path, &ListQuery::default()).await?;
        Ok(record)
    }

    /// `POST /zones/{zone_id}/dns_records` — create a DNS record.
    pub async fn create_dns_record(&self, zone_id: &str, body: &DnsRecord) -> Result<DnsRecord> {
        let path = format!("/zones/{}/dns_records", self.encode_path(zone_id));
        self.post_json(&path, body).await
    }

    /// `PUT /zones/{zone_id}/dns_records/{id}` — replace a DNS record.
    pub async fn update_dns_record(
        &self,
        zone_id: &str,
        record_id: &str,
        body: &DnsRecord,
    ) -> Result<DnsRecord> {
        let path = format!(
            "/zones/{}/dns_records/{}",
            self.encode_path(zone_id),
            self.encode_path(record_id)
        );
        self.put_json(&path, body).await
    }

    /// `DELETE /zones/{zone_id}/dns_records/{id}` — delete a DNS record.
    pub async fn delete_dns_record(&self, zone_id: &str, record_id: &str) -> Result<()> {
        let path = format!(
            "/zones/{}/dns_records/{}",
            self.encode_path(zone_id),
            self.encode_path(record_id)
        );
        self.delete(&path).await
    }
}
