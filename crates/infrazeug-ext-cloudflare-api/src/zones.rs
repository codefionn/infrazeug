//! Zone management (`/zones`).

use crate::client::CloudflareClient;
use crate::error::{CloudflareError, Result};
use crate::types::ListQuery;
use serde::{Deserialize, Serialize};

/// A DNS zone in the Cloudflare account.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Zone {
    pub id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<ZoneAccount>,
}

/// Owning account reference on a zone.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ZoneAccount {
    pub id: Option<String>,
    pub name: Option<String>,
}

impl CloudflareClient {
    /// `GET /zones` — list zones visible to the token (all pages).
    pub async fn zones(&self, query: &ListQuery) -> Result<Vec<Zone>> {
        self.get_all("/zones", query.clone()).await
    }

    /// `GET /zones/{id}` — fetch one zone by id.
    pub async fn zone(&self, zone_id: &str) -> Result<Zone> {
        let path = format!("/zones/{}", self.encode_path(zone_id));
        let (zone, _) = self.get(&path, &ListQuery::default()).await?;
        Ok(zone)
    }

    /// Resolve a zone id from its DNS name (exact match, active zones).
    pub async fn zone_id_by_name(&self, name: &str) -> Result<String> {
        let query = ListQuery {
            name: Some(name.into()),
            status: Some("active".into()),
            per_page: Some(50),
            ..Default::default()
        };
        let zones = self.zones(&query).await?;
        zones
            .into_iter()
            .find(|z| z.name.eq_ignore_ascii_case(name))
            .and_then(|z| z.id)
            .ok_or_else(|| CloudflareError::Api {
                status: 404,
                codes: vec![],
                message: format!("zone not found: {name}"),
            })
    }
}
