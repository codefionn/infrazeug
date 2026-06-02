//! DNS zones (`/domain/zone`).

use super::zone_path;
use crate::client::OvhClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// DNS record type (`domain.zone.RecordTypeEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum DnsRecordType {
    #[serde(rename = "A")]
    A,
    #[serde(rename = "AAAA")]
    Aaaa,
    #[serde(rename = "CAA")]
    Caa,
    #[serde(rename = "CNAME")]
    Cname,
    #[serde(rename = "DKIM")]
    Dkim,
    #[serde(rename = "DMARC")]
    Dmarc,
    #[serde(rename = "DNAME")]
    Dname,
    #[serde(rename = "HTTPS")]
    Https,
    #[serde(rename = "LOC")]
    Loc,
    #[serde(rename = "MX")]
    Mx,
    #[serde(rename = "NAPTR")]
    Naptr,
    #[serde(rename = "NS")]
    Ns,
    #[serde(rename = "PTR")]
    Ptr,
    #[serde(rename = "RP")]
    Rp,
    #[serde(rename = "SPF")]
    Spf,
    #[serde(rename = "SRV")]
    Srv,
    #[serde(rename = "SSHFP")]
    Sshfp,
    #[serde(rename = "SVCB")]
    Svcb,
    #[serde(rename = "TLSA")]
    Tlsa,
    #[serde(rename = "TXT")]
    Txt,
    #[serde(other)]
    Unknown,
}

/// Filters for `GET /domain/zone/{zoneName}/record`.
#[derive(Debug, Clone, Default)]
pub struct ZoneRecordListQuery<'a> {
    pub field_type: Option<DnsRecordType>,
    pub sub_domain: Option<&'a str>,
}

/// A DNS zone (`domain.Zone`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsZone {
    pub name: String,
    pub name_servers: Vec<String>,
    pub dnssec_activated: bool,
    pub dnssec_supported: bool,
    pub has_dns_anycast: bool,
    #[serde(default)]
    pub last_update: Option<String>,
}

/// A DNS zone record (`domain.zone.Record`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsZoneRecord {
    pub id: i64,
    pub zone: String,
    pub field_type: DnsRecordType,
    pub target: String,
    #[serde(default)]
    pub sub_domain: Option<String>,
    #[serde(default)]
    pub ttl: Option<i64>,
}

/// Body for `POST /domain/zone/{zoneName}/record`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsZoneRecordCreate {
    pub field_type: DnsRecordType,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<i64>,
}

/// Body for `PUT /domain/zone/{zoneName}/record/{id}`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsZoneRecordUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<i64>,
}

impl OvhClient {
    /// `GET /domain/zone` — list DNS zone names.
    pub async fn domain_zones(&self) -> Result<Vec<String>> {
        self.get_v1("/domain/zone").await
    }

    /// `GET /domain/zone/{zoneName}` — zone details.
    pub async fn domain_zone(&self, zone_name: &str) -> Result<DnsZone> {
        let path = zone_path(self, zone_name, "");
        self.get_v1(&path).await
    }

    /// `POST /domain/zone/{zoneName}/refresh` — refresh zone from registry.
    pub async fn domain_zone_refresh(&self, zone_name: &str) -> Result<()> {
        let path = zone_path(self, zone_name, "/refresh");
        self.post_v1_void(&path).await
    }

    /// `GET /domain/zone/{zoneName}/record` — list record ids.
    pub async fn domain_zone_record_ids(
        &self,
        zone_name: &str,
        query: ZoneRecordListQuery<'_>,
    ) -> Result<Vec<i64>> {
        let mut path = zone_path(self, zone_name, "/record");
        let mut params = Vec::new();
        if let Some(field_type) = query.field_type {
            let value = serde_json::to_string(&field_type)?;
            params.push(("fieldType".to_string(), trim_json_string(&value)));
        }
        if let Some(sub) = query.sub_domain {
            params.push(("subDomain".to_string(), sub.to_string()));
        }
        if !params.is_empty() {
            let pairs: Vec<(&str, &str)> = params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            path = OvhClient::append_query(&path, &pairs);
        }
        self.get_v1_url(&path).await
    }

    /// `GET /domain/zone/{zoneName}/record/{id}` — fetch one record.
    pub async fn domain_zone_record(
        &self,
        zone_name: &str,
        record_id: i64,
    ) -> Result<DnsZoneRecord> {
        let path = format!("{}/record/{record_id}", zone_path(self, zone_name, ""));
        self.get_v1(&path).await
    }

    /// `POST /domain/zone/{zoneName}/record` — create a record.
    pub async fn domain_zone_record_create(
        &self,
        zone_name: &str,
        create: &DnsZoneRecordCreate,
    ) -> Result<DnsZoneRecord> {
        let path = zone_path(self, zone_name, "/record");
        self.post_v1(&path, create).await
    }

    /// `PUT /domain/zone/{zoneName}/record/{id}` — update a record.
    pub async fn domain_zone_record_update(
        &self,
        zone_name: &str,
        record_id: i64,
        update: &DnsZoneRecordUpdate,
    ) -> Result<DnsZoneRecord> {
        let path = format!("{}/record/{record_id}", zone_path(self, zone_name, ""));
        self.put_v1_typed(&path, update).await
    }

    /// `DELETE /domain/zone/{zoneName}/record/{id}`.
    pub async fn domain_zone_record_delete(&self, zone_name: &str, record_id: i64) -> Result<()> {
        let path = format!("{}/record/{record_id}", zone_path(self, zone_name, ""));
        self.delete_v1(&path).await
    }
}

fn trim_json_string(s: &str) -> String {
    s.trim_matches('"').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_zone_record() {
        let rec: DnsZoneRecord = serde_json::from_str(
            r#"{
                "id": 1,
                "zone": "example.com",
                "fieldType": "A",
                "target": "203.0.113.1",
                "subDomain": "www",
                "ttl": 3600
            }"#,
        )
        .unwrap();
        assert_eq!(rec.field_type, DnsRecordType::A);
    }
}
