//! OVHcloud **allDom** product bindings (API v1 `/allDom`).

use crate::client::OvhClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// AllDom service offer tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AllDomOffer {
    Diamond,
    Gold,
    Platinum,
}

/// AllDom geographic coverage type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum AllDomType {
    #[serde(rename = "french")]
    French,
    #[serde(rename = "french+international")]
    FrenchInternational,
    #[serde(rename = "international")]
    International,
}

/// General information about an AllDom service (v1).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllDomService {
    pub name: String,
    pub offer: AllDomOffer,
    #[serde(rename = "type")]
    pub service_type: AllDomType,
}

/// A domain name attached to an AllDom pack.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllDomDomain {
    pub domain: String,
}

/// Renewal settings for a billable service.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenewSettings {
    pub automatic: Option<bool>,
    pub delete_at_expiration: Option<bool>,
    pub forced: Option<bool>,
    pub manual_payment: Option<bool>,
    pub period: Option<i64>,
}

/// Billing / lifecycle metadata for a service (`services.Service`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfos {
    pub can_delete_at_expiration: bool,
    pub contact_admin: String,
    pub contact_billing: String,
    pub contact_tech: String,
    pub creation: String,
    pub domain: String,
    pub engaged_up_to: Option<String>,
    pub expiration: String,
    pub possible_renew_period: Option<Vec<i64>>,
    pub renew: Option<RenewSettings>,
    pub renewal_type: String,
    pub service_id: i64,
    pub status: String,
}

impl OvhClient {
    /// `GET /allDom` — list AllDom service names for the authenticated account.
    pub async fn alldom_services(&self) -> Result<Vec<String>> {
        self.get_v1("/allDom").await
    }

    /// `GET /allDom/{serviceName}` — fetch AllDom service properties.
    pub async fn alldom_service(&self, service_name: &str) -> Result<AllDomService> {
        let path = format!("/allDom/{}", self.encode_segment(service_name));
        self.get_v1(&path).await
    }

    /// `GET /allDom/{serviceName}/domain` — list domain names on the pack.
    pub async fn alldom_domains(&self, service_name: &str) -> Result<Vec<String>> {
        let path = format!("/allDom/{}/domain", self.encode_segment(service_name));
        self.get_v1(&path).await
    }

    /// `GET /allDom/{serviceName}/domain/{domain}` — fetch one attached domain.
    pub async fn alldom_domain(&self, service_name: &str, domain: &str) -> Result<AllDomDomain> {
        let path = format!(
            "/allDom/{}/domain/{}",
            self.encode_segment(service_name),
            self.encode_segment(domain),
        );
        self.get_v1(&path).await
    }

    /// `GET /allDom/{serviceName}/serviceInfos` — billing and renewal metadata.
    pub async fn alldom_service_infos(&self, service_name: &str) -> Result<ServiceInfos> {
        let path = format!("/allDom/{}/serviceInfos", self.encode_segment(service_name));
        self.get_v1(&path).await
    }

    /// `PUT /allDom/{serviceName}/serviceInfos` — update renewal settings.
    pub async fn alldom_update_service_infos(
        &self,
        service_name: &str,
        infos: &ServiceInfos,
    ) -> Result<()> {
        let path = format!("/allDom/{}/serviceInfos", self.encode_segment(service_name));
        self.put_v1(&path, infos).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_service_type_variants() {
        let french: AllDomType = serde_json::from_str(r#""french""#).unwrap();
        assert_eq!(french, AllDomType::French);

        let mixed: AllDomType = serde_json::from_str(r#""french+international""#).unwrap();
        assert_eq!(mixed, AllDomType::FrenchInternational);
    }
}
