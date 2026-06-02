//! OVHcloud API v2 **videocenter** bindings (`/v2/videocenter`).
//!
//! Generated from the official schema; do not edit by hand.

#![allow(unused_imports, unused_variables)]

use crate::client::{percent_encode, OvhClient, PageParams, V2RequestOptions};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// `common.ResourceStatusEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommonResourceStatus {
    #[serde(rename = "CREATING")]
    Creating,
    #[serde(rename = "DELETING")]
    Deleting,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "OUT_OF_SYNC")]
    OutOfSync,
    #[serde(rename = "READY")]
    Ready,
    #[serde(rename = "SUSPENDED")]
    Suspended,
    #[serde(rename = "UNKNOWN")]
    UnknownValue,
    #[serde(rename = "UPDATING")]
    Updating,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `iam.resource.TagFilter.OperatorEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IamResourceTagFilterOperator {
    #[serde(rename = "EQ")]
    Eq,
    #[serde(rename = "EXISTS")]
    Exists,
    #[serde(rename = "ILIKE")]
    Ilike,
    #[serde(rename = "LIKE")]
    Like,
    #[serde(rename = "NEQ")]
    Neq,
    #[serde(rename = "NEXISTS")]
    Nexists,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `iam.resource.TagFilter`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTagFilter {
    pub operator: Option<IamResourceTagFilterOperator>,
    pub value: Option<String>,
}

/// `videocenter.AuthTokenRequest`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthTokenRequest {
    pub language: String,
}

/// `videocenter.AuthTokenResponse`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthTokenResponse {
    pub token: Option<String>,
}

/// `videocenter.ServiceCurrentStateVodCount`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCurrentStateVodCount {
    pub allocated: Option<i64>,
    pub hostable: Option<i64>,
}

/// `videocenter.ServiceCurrentStateVodDurationMinutes`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCurrentStateVodDurationMinutes {
    pub allocated: Option<f64>,
    pub hostable: Option<i64>,
}

/// `videocenter.ServiceCurrentState`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCurrentState {
    pub created_at: Option<String>,
    pub offer_name: Option<String>,
    pub vod_count: Option<ServiceCurrentStateVodCount>,
    pub vod_duration_minutes: Option<ServiceCurrentStateVodDurationMinutes>,
}

/// `videocenter.Service`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub current_state: Option<ServiceCurrentState>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
}

/// `videocenter.ServiceWithIAM`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceWithIAM {
    pub current_state: Option<ServiceCurrentState>,
    pub iam: Option<crate::iam::ResourceMetadata>,
    pub id: Option<String>,
    pub resource_status: Option<CommonResourceStatus>,
}

impl OvhClient {
    /// `GET /videocenter/resource` — Get all services
    pub async fn videocenter_resources(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<ServiceWithIAM>> {
        self.get_page(
            &Self::append_query("/videocenter/resource", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /videocenter/resource/{serviceId}` — Get a service
    pub async fn videocenter_resource(&self, service_id: &str) -> Result<ServiceWithIAM> {
        self.get(&format!(
            "/videocenter/resource/{}",
            percent_encode(service_id)
        ))
        .await
    }

    /// `POST /videocenter/resource/{serviceId}/auth/token` — Generate an Auth Token
    pub async fn videocenter_resource_auth_token_post(
        &self,
        service_id: &str,
        body: &AuthTokenRequest,
    ) -> Result<AuthTokenResponse> {
        self.post_v2(
            &format!(
                "/videocenter/resource/{}/auth/token",
                percent_encode(service_id)
            ),
            body,
            V2RequestOptions::default(),
        )
        .await
    }
}
