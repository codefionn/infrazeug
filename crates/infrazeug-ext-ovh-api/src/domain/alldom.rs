//! API v2 AllDom resources (`/domain/alldom`).

use super::ResourceStatus;
use crate::client::{OvhClient, V2PageInfo, V2RequestOptions};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// AllDom geographic coverage (v2 naming).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum V2AllDomType {
    French,
    #[serde(rename = "FRENCH+INTERNATIONAL")]
    FrenchInternational,
    International,
}

/// Domain registration status within an AllDom pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RegistrationStatus {
    Registered,
    Unregistered,
}

/// A domain entry in an AllDom pack's current state.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2AllDomDomainEntry {
    pub name: String,
    pub extension: Option<String>,
    pub main_state: Option<String>,
    pub protection_state: Option<String>,
    pub suspension_state: Option<String>,
    pub registration_status: RegistrationStatus,
    pub expires_at: Option<String>,
    pub dnssec_activated: Option<bool>,
}

/// Current state snapshot of an AllDom resource.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2AllDomCurrentState {
    pub name: String,
    pub r#type: V2AllDomType,
    pub extensions: Vec<String>,
    pub domains: Vec<V2AllDomDomainEntry>,
}

/// An AllDom resource (`GET /domain/alldom/{alldomName}`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2AllDom {
    pub id: String,
    pub checksum: String,
    pub resource_status: ResourceStatus,
    pub current_state: V2AllDomCurrentState,
}

/// Paginated `GET /domain/alldom` response wrapper.
#[derive(Debug, Clone)]
pub struct V2AllDomList {
    pub items: Vec<V2AllDom>,
    pub page: V2PageInfo,
}

impl OvhClient {
    /// `GET /domain/alldom` — list AllDom resources (API v2).
    pub async fn domain_alldom_list(&self, options: V2RequestOptions<'_>) -> Result<V2AllDomList> {
        let (items, page) = self.get_v2("/domain/alldom", options).await?;
        Ok(V2AllDomList { items, page })
    }

    /// `GET /domain/alldom/{alldomName}` — fetch one AllDom resource.
    pub async fn domain_alldom(&self, alldom_name: &str) -> Result<V2AllDom> {
        let path = format!("/domain/alldom/{}", self.encode_segment(alldom_name));
        let (resource, _) = self.get_v2(&path, V2RequestOptions::default()).await?;
        Ok(resource)
    }
}
