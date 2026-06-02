//! Account identity (`GET /me`).

use crate::client::OvhClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// OVH account / nichandle summary (`nichandle.Nichandle`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub nichandle: String,
    pub email: String,
    pub firstname: String,
    pub name: String,
    pub country: String,
    pub language: String,
    pub customer_code: Option<String>,
    pub organisation: Option<String>,
    pub phone: Option<String>,
    pub spare_email: Option<String>,
}

impl OvhClient {
    /// `GET /me` — authenticated account details (handy to verify credentials).
    pub async fn me(&self) -> Result<Account> {
        self.get_v1("/me").await
    }
}
