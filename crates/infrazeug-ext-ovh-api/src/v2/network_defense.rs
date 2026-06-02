//! OVHcloud API v2 **networkDefense** bindings (`/v2/networkDefense`).
//!
//! Generated from the official schema; do not edit by hand.

#![allow(unused_imports, unused_variables)]

use crate::client::{percent_encode, OvhClient, PageParams, V2RequestOptions};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// `networkDefense.RegionEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NetworkDefenseRegion {
    #[serde(rename = "CA")]
    Ca,
    #[serde(rename = "EU")]
    Eu,
    #[serde(rename = "US")]
    Us,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `networkDefense.VectorsEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NetworkDefenseVectors {
    #[serde(rename = "CHARGEN")]
    Chargen,
    #[serde(rename = "DNS")]
    Dns,
    #[serde(rename = "DNS_TO_OVH")]
    DnsToOvh,
    #[serde(rename = "FRAGMENT")]
    Fragment,
    #[serde(rename = "ICMP")]
    Icmp,
    #[serde(rename = "IP_NULL")]
    IpNull,
    #[serde(rename = "NTP")]
    Ntp,
    #[serde(rename = "OTHER")]
    Other,
    #[serde(rename = "TCP_ACK")]
    TcpAck,
    #[serde(rename = "TCP_FIN")]
    TcpFin,
    #[serde(rename = "TCP_NULL")]
    TcpNull,
    #[serde(rename = "TCP_PSH")]
    TcpPsh,
    #[serde(rename = "TCP_RST")]
    TcpRst,
    #[serde(rename = "TCP_SYN")]
    TcpSyn,
    #[serde(rename = "UDP")]
    Udp,
    #[serde(rename = "VECTOR_TYPE_UNSPECIFIED")]
    VectorTypeUnspecified,
}

/// `networkDefense.Vac.Event`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VacEvent {
    pub ended_at: Option<String>,
    pub started_at: Option<String>,
    pub subnet: Option<String>,
    pub vectors: Option<Vec<NetworkDefenseVectors>>,
}

/// `networkDefense.Vac.EventsResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VacEventsResponse {
    pub events: Option<Vec<VacEvent>>,
}

/// `networkDefense.Vac.TrafficResponseData`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VacTrafficResponseData {
    pub dropped: Option<Vec<String>>,
    pub passed: Option<Vec<String>>,
}

/// `networkDefense.Vac.TrafficResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VacTrafficResponse {
    pub bps: Option<VacTrafficResponseData>,
    pub pps: Option<VacTrafficResponseData>,
    pub timestamps: Option<Vec<String>>,
}

impl OvhClient {
    /// `GET /networkDefense/vac/event` — Get all Network Defense events
    pub async fn network_defense_vac_events(
        &self,
        query: &[(&str, &str)],
    ) -> Result<VacEventsResponse> {
        self.get(&Self::append_query("/networkDefense/vac/event", query))
            .await
    }

    /// `GET /networkDefense/vac/traffic` — Get all Network Defense traffic statistics
    pub async fn network_defense_vac_traffics(
        &self,
        query: &[(&str, &str)],
    ) -> Result<VacTrafficResponse> {
        self.get(&Self::append_query("/networkDefense/vac/traffic", query))
            .await
    }
}
