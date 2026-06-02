//! OVHcloud API v2 **location** bindings (`/v2/location`).
//!
//! Generated from the official schema; do not edit by hand.

#![allow(unused_imports, unused_variables)]

use crate::client::{percent_encode, OvhClient, PageParams, V2RequestOptions};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// `common.LanguageEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommonLanguage {
    #[serde(rename = "cs_CZ")]
    CsCz,
    #[serde(rename = "de_DE")]
    DeDe,
    #[serde(rename = "en_AS")]
    EnAs,
    #[serde(rename = "en_AU")]
    EnAu,
    #[serde(rename = "en_CA")]
    EnCa,
    #[serde(rename = "en_GB")]
    EnGb,
    #[serde(rename = "en_IE")]
    EnIe,
    #[serde(rename = "en_IN")]
    EnIn,
    #[serde(rename = "en_SG")]
    EnSg,
    #[serde(rename = "en_US")]
    EnUs,
    #[serde(rename = "en_WW")]
    EnWw,
    #[serde(rename = "es_ES")]
    EsEs,
    #[serde(rename = "es_SA")]
    EsSa,
    #[serde(rename = "fi_FI")]
    FiFi,
    #[serde(rename = "fr_CA")]
    FrCa,
    #[serde(rename = "fr_FR")]
    FrFr,
    #[serde(rename = "fr_MA")]
    FrMa,
    #[serde(rename = "fr_SN")]
    FrSn,
    #[serde(rename = "fr_TN")]
    FrTn,
    #[serde(rename = "it_IT")]
    ItIt,
    #[serde(rename = "lt_LT")]
    LtLt,
    #[serde(rename = "nl_NL")]
    NlNl,
    #[serde(rename = "pl_PL")]
    PlPl,
    #[serde(rename = "pt_PT")]
    PtPt,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `location.CardinalPointEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocationCardinalPoint {
    #[serde(rename = "CENTRAL")]
    Central,
    #[serde(rename = "EAST")]
    East,
    #[serde(rename = "NORTH")]
    North,
    #[serde(rename = "NORTHEAST")]
    Northeast,
    #[serde(rename = "NORTHWEST")]
    Northwest,
    #[serde(rename = "SOUTH")]
    South,
    #[serde(rename = "SOUTHEAST")]
    Southeast,
    #[serde(rename = "SOUTHWEST")]
    Southwest,
    #[serde(rename = "WEST")]
    West,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `location.ServicesEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocationServices {
    #[serde(rename = "OCC")]
    Occ,
    #[serde(rename = "PEERING")]
    Peering,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `location.SpecificTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocationSpecificType {
    #[serde(rename = "BACKUP")]
    Backup,
    #[serde(rename = "LZ")]
    Lz,
    #[serde(rename = "SNC")]
    Snc,
    #[serde(rename = "STANDARD")]
    Standard,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `location.TypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocationType {
    #[serde(rename = "LOCAL-ZONE")]
    LocalZone,
    #[serde(rename = "REGION-1-AZ")]
    Region1Az,
    #[serde(rename = "REGION-3-AZ")]
    Region3Az,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `location.Location`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    #[serde(default)]
    pub availability_zone_datacenters: serde_json::Value,
    pub availability_zones: Option<Vec<String>>,
    pub cardinal_point: Option<LocationCardinalPoint>,
    pub city_code: Option<String>,
    pub city_latitude: Option<f64>,
    pub city_longitude: Option<f64>,
    pub city_name: Option<String>,
    pub code: Option<String>,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub geography_code: Option<String>,
    pub geography_name: Option<String>,
    pub location: Option<String>,
    pub name: Option<String>,
    pub opening_year: Option<i64>,
    pub services: Option<Vec<LocationServices>>,
    pub specific_type: Option<LocationSpecificType>,
    #[serde(rename = "type")]
    pub kind: Option<LocationType>,
}

impl OvhClient {
    /// `GET /location` — List available regions and their availability zones
    pub async fn locations(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<Location>> {
        self.get_page(&Self::append_query("/location", query), &[], page)
            .await
            .map(|p| p.items)
    }

    /// `GET /location/{name}` — Get available region and its availability zones
    pub async fn location_location(&self, name: &str) -> Result<Location> {
        self.get(&format!("/location/{}", percent_encode(name)))
            .await
    }
}
