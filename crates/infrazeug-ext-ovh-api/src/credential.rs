//! Consumer-key bootstrap (`POST /auth/credential`).

use crate::client::OvhClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// HTTP verb allowed in a credential access rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

/// One API route/grant requested for a new consumer key.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AccessRule {
    pub method: HttpMethod,
    pub path: String,
}

impl AccessRule {
    /// `GET` on `path`.
    pub fn get(path: impl Into<String>) -> Self {
        Self {
            method: HttpMethod::Get,
            path: path.into(),
        }
    }

    /// `POST` on `path`.
    pub fn post(path: impl Into<String>) -> Self {
        Self {
            method: HttpMethod::Post,
            path: path.into(),
        }
    }

    /// `PUT` on `path`.
    pub fn put(path: impl Into<String>) -> Self {
        Self {
            method: HttpMethod::Put,
            path: path.into(),
        }
    }

    /// `DELETE` on `path`.
    pub fn delete(path: impl Into<String>) -> Self {
        Self {
            method: HttpMethod::Delete,
            path: path.into(),
        }
    }
}

/// Body for `POST /auth/credential`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRequest {
    pub access_rules: Vec<AccessRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirection: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_ips: Option<Vec<String>>,
}

/// Credential lifecycle state returned by OVH.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialState {
    Expired,
    PendingValidation,
    Refused,
    Validated,
}

/// Response from `POST /auth/credential`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRequestResult {
    pub consumer_key: String,
    pub state: CredentialState,
    pub validation_url: String,
}

impl OvhClient {
    /// `POST /auth/credential` — request a new consumer key (application credentials only).
    ///
    /// Build the client with [`crate::Credentials::application_only`] (or omit the
    /// consumer key). The end user must visit
    /// [`CredentialRequestResult::validation_url`] to approve the requested
    /// [`CredentialRequest::access_rules`].
    pub async fn request_consumer_key(
        &self,
        request: &CredentialRequest,
    ) -> Result<CredentialRequestResult> {
        self.post_v1_public("/auth/credential", request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_access_rules() {
        let body = CredentialRequest {
            access_rules: vec![AccessRule::get("/me"), AccessRule::get("/allDom/*")],
            redirection: None,
            allowed_ips: None,
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains(r#""method":"GET""#));
        assert!(json.contains(r#""path":"/me""#));
    }
}
