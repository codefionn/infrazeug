//! Local DNS records (v2 API `static-dns`).
//!
//! UniFi's controller-managed DNS records live on the newer v2 API, so these bind
//! to [`UnifiClient::v2_list`](crate::UnifiClient) and friends rather than the
//! classic `/rest` surface.

use crate::client::UnifiClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

const RESOURCE: &str = "static-dns";

/// A local DNS record served by the controller's resolver.
///
/// Unmodelled fields round-trip through [`extra`](Self::extra).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DnsRecord {
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Record name / hostname being resolved (e.g. `nas.lan`). For `PTR` records
    /// this is the reverse-lookup key.
    pub key: String,
    /// Record type: `A`, `AAAA`, `CNAME`, `TXT`, `MX`, `SRV`, `NS`, `PTR`.
    pub record_type: String,
    /// Record value (target IP, hostname, text, …).
    pub value: String,
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
    /// Fields this client does not model, preserved verbatim across updates.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl UnifiClient {
    /// `GET /v2/.../static-dns` — list local DNS records on the site.
    pub async fn dns_records(&self) -> Result<Vec<DnsRecord>> {
        self.v2_list(RESOURCE).await
    }

    /// `POST /v2/.../static-dns` — create a local DNS record.
    pub async fn create_dns_record(&self, body: &DnsRecord) -> Result<DnsRecord> {
        self.v2_create(RESOURCE, body).await
    }

    /// `PUT /v2/.../static-dns/{id}` — replace a local DNS record.
    pub async fn update_dns_record(&self, id: &str, body: &DnsRecord) -> Result<DnsRecord> {
        self.v2_update(&format!("{RESOURCE}/{id}"), body).await
    }

    /// `DELETE /v2/.../static-dns/{id}` — delete a local DNS record.
    pub async fn delete_dns_record(&self, id: &str) -> Result<()> {
        self.v2_delete(&format!("{RESOURCE}/{id}")).await
    }
}
