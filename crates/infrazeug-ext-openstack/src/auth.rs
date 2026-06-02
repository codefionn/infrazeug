//! Keystone v3 password authentication types.

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Non-secret connection parameters for an OpenStack project.
#[derive(Clone, Debug)]
pub struct OpenstackConfig {
    /// Keystone auth URL (e.g. `https://auth.cloud.ovh.net/v3`).
    pub auth_url: String,
    /// OpenStack project id (tenant scope for auth and EC2 creds).
    pub project_id: String,
    /// Region name for catalog lookup (e.g. `DE`).
    pub region: String,
    /// User domain name (OVH Public Cloud: `Default`).
    pub domain: String,
}

impl OpenstackConfig {
    pub fn ovh_public_cloud(project_id: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            auth_url: "https://auth.cloud.ovh.net/v3".into(),
            project_id: project_id.into(),
            region: region.into(),
            domain: "Default".into(),
        }
    }
}

/// Successful Keystone token response (body of `POST …/auth/tokens`).
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub token: TokenBody,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenBody {
    pub expires_at: DateTime<Utc>,
    pub user: TokenUser,
    #[serde(default)]
    pub catalog: Vec<CatalogEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenUser {
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogEntry {
    #[serde(rename = "type")]
    pub service_type: String,
    #[serde(default)]
    pub endpoints: Vec<CatalogEndpoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogEndpoint {
    pub url: String,
    pub interface: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub region_id: Option<String>,
}

/// Resolve a public endpoint URL from the service catalog.
pub fn catalog_endpoint(
    catalog: &[CatalogEntry],
    service_type: &str,
    region: &str,
) -> Option<String> {
    let region_upper = region.to_ascii_uppercase();
    for entry in catalog {
        if entry.service_type != service_type {
            continue;
        }
        for ep in &entry.endpoints {
            if ep.interface != "public" {
                continue;
            }
            let matches = ep
                .region
                .as_deref()
                .map(|r| r.eq_ignore_ascii_case(&region_upper))
                .unwrap_or(false)
                || ep
                    .region_id
                    .as_deref()
                    .map(|r| r.eq_ignore_ascii_case(&region_upper))
                    .unwrap_or(false);
            if matches {
                return Some(ep.url.trim_end_matches('/').to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_token_response() {
        let body = r#"{
            "token": {
                "expires_at": "2024-06-01T12:00:00.000000Z",
                "user": { "id": "uid-1", "name": "user-abc" },
                "catalog": [{
                    "type": "object-store",
                    "endpoints": [{
                        "url": "https://storage.de.cloud.ovh.net/v1/AUTH_proj",
                        "interface": "public",
                        "region": "DE"
                    }]
                }]
            }
        }"#;
        let parsed: TokenResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.token.user.id, "uid-1");
        assert_eq!(parsed.token.catalog.len(), 1);
        assert_eq!(
            catalog_endpoint(&parsed.token.catalog, "object-store", "DE"),
            Some("https://storage.de.cloud.ovh.net/v1/AUTH_proj".into())
        );
    }
}
