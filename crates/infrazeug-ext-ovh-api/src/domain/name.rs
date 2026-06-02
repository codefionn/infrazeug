//! API v2 domain name resources (`/domain/name`).

use super::ResourceStatus;
use crate::client::{OvhClient, V2PageInfo, V2RequestOptions};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Main lifecycle state of a domain name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DomainMainState {
    Deleted,
    Expired,
    Ok,
    PendingCreate,
    PendingDelete,
    PendingInternalTransfer,
    PendingOutgoingTransfer,
    Restorable,
    ToDelete,
    #[serde(other)]
    Unknown,
}

/// Protection state of a domain name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProtectionState {
    Protected,
    Unprotected,
}

/// Suspension state of a domain name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SuspensionState {
    NotSuspended,
    Suspended,
}

/// Disclosure policy for a contact (`domain.resource.DisclosureConfigurationEnum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DisclosureConfiguration {
    Disclosed,
    Redacted,
}

/// Optional filters for `GET /domain/name`.
#[derive(Debug, Clone, Default)]
pub struct DomainNameListQuery<'a> {
    pub search_value: Option<&'a str>,
    pub main_state: Option<&'a [DomainMainState]>,
    pub suspension_state: Option<&'a [SuspensionState]>,
    pub contact_administrator: Option<&'a [&'a str]>,
    pub contact_billing: Option<&'a [&'a str]>,
    pub contact_owner: Option<&'a [&'a str]>,
    pub contact_technical: Option<&'a [&'a str]>,
    pub options: V2RequestOptions<'a>,
}

/// Contacts on a domain (current state).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2ContactsConfiguration {
    #[serde(default)]
    pub contact_administrator: Option<V2ContactConfiguration>,
    #[serde(default)]
    pub contact_billing: Option<V2ContactConfiguration>,
    #[serde(default)]
    pub contact_owner: Option<V2ContactConfiguration>,
    #[serde(default)]
    pub contact_technical: Option<V2ContactConfiguration>,
}

/// One contact slot in current state.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2ContactConfiguration {
    pub id: String,
}

/// Target disclosure policy for a contact.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2DisclosurePolicy {
    pub disclosure_configuration: DisclosureConfiguration,
}

/// Target contact configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2ContactTargetConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disclosure_policy: Option<V2DisclosurePolicy>,
}

/// Target owner contact (allows id change).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2OwnerTargetConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disclosure_policy: Option<V2DisclosurePolicy>,
}

/// Target contacts specification.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2ContactsTargetConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_administrator: Option<V2ContactTargetConfiguration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_billing: Option<V2ContactTargetConfiguration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_owner: Option<V2OwnerTargetConfiguration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_technical: Option<V2ContactTargetConfiguration>,
}

/// Name server in current or target DNS config.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2NameServer {
    pub name_server: String,
    #[serde(default)]
    pub ipv4: Option<String>,
    #[serde(default)]
    pub ipv6: Option<String>,
}

/// DNS configuration in current state.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2DnsCurrentConfiguration {
    #[serde(default)]
    pub name_servers: Vec<V2NameServer>,
    #[serde(default)]
    pub configuration_type: Option<String>,
    #[serde(default)]
    pub min_dns: Option<i64>,
    #[serde(default)]
    pub max_dns: Option<i64>,
}

/// DNS target configuration (`targetSpec.dnsConfiguration`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2DnsTargetConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_servers: Option<Vec<V2NameServer>>,
}

/// DS data entry for DNSSEC.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2DsData {
    pub algorithm: i64,
    pub flags: i64,
    pub key_tag: i64,
    pub public_key: String,
}

/// DNSSEC target configuration.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2DnssecTargetConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ds_data: Option<Vec<V2DsData>>,
}

/// In-flight task summary on a resource (`common.CurrentTask`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2CurrentTask {
    pub id: String,
    pub r#type: String,
    pub link: String,
    #[serde(default)]
    pub status: Option<String>,
}

/// Current state of a domain name resource.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2DomainCurrentState {
    pub name: String,
    pub extension: String,
    pub main_state: DomainMainState,
    pub protection_state: ProtectionState,
    pub suspension_state: SuspensionState,
    pub created_at: String,
    #[serde(default)]
    pub additional_states: Vec<String>,
    #[serde(default)]
    pub contacts_configuration: Option<V2ContactsConfiguration>,
    #[serde(default)]
    pub dns_configuration: Option<V2DnsCurrentConfiguration>,
    #[serde(default)]
    pub auth_info_supported: Option<bool>,
    #[serde(default)]
    pub auth_info_managed_by_ovhcloud: Option<bool>,
}

/// Domain target specification for `PUT /domain/name/{domainName}`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2DomainTargetSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contacts_configuration: Option<V2ContactsTargetConfiguration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns_configuration: Option<V2DnsTargetConfiguration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dnssec_configuration: Option<V2DnssecTargetConfiguration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protection_state: Option<ProtectionState>,
}

/// A domain name resource (`GET /domain/name/{domainName}`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2Domain {
    pub id: String,
    pub checksum: String,
    pub resource_status: ResourceStatus,
    pub current_state: V2DomainCurrentState,
    pub target_spec: V2DomainTargetSpec,
    #[serde(default)]
    pub current_tasks: Vec<V2CurrentTask>,
}

/// Body for `PUT /domain/name/{domainName}` — checksum + desired target spec.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct V2DomainUpdate {
    pub checksum: String,
    pub target_spec: V2DomainTargetSpec,
}

/// Paginated `GET /domain/name` response wrapper.
#[derive(Debug, Clone)]
pub struct V2DomainList {
    pub items: Vec<V2Domain>,
    pub page: V2PageInfo,
}

impl OvhClient {
    /// `GET /domain/name` — list domain name resources (API v2).
    pub async fn domain_name_list(&self, query: DomainNameListQuery<'_>) -> Result<V2DomainList> {
        let params = build_name_list_params(&query)?;
        let path = build_query_path("/domain/name", &params);
        let (items, page) = self.get_v2_url(&path, query.options).await?;
        Ok(V2DomainList { items, page })
    }

    /// `GET /domain/name/{domainName}` — fetch one domain name resource.
    pub async fn domain_name(&self, domain_name: &str) -> Result<V2Domain> {
        let path = format!("/domain/name/{}", self.encode_segment(domain_name));
        let (resource, _) = self.get_v2(&path, V2RequestOptions::default()).await?;
        Ok(resource)
    }

    /// `PUT /domain/name/{domainName}` — apply a new target specification.
    pub async fn domain_name_update(
        &self,
        domain_name: &str,
        update: &V2DomainUpdate,
        options: V2RequestOptions<'_>,
    ) -> Result<V2Domain> {
        let path = format!("/domain/name/{}", self.encode_segment(domain_name));
        self.put_v2(&path, update, options).await
    }
}

fn build_name_list_params(query: &DomainNameListQuery<'_>) -> Result<Vec<(String, String)>> {
    let mut params: Vec<(String, String)> = Vec::new();
    if let Some(search) = query.search_value {
        params.push(("searchValue".into(), search.into()));
    }
    append_enum_list(&mut params, "mainState", query.main_state)?;
    append_enum_list(&mut params, "suspensionState", query.suspension_state)?;
    append_str_list(
        &mut params,
        "contactAdministrator",
        query.contact_administrator,
    );
    append_str_list(&mut params, "contactBilling", query.contact_billing);
    append_str_list(&mut params, "contactOwner", query.contact_owner);
    append_str_list(&mut params, "contactTechnical", query.contact_technical);
    Ok(params)
}

fn append_enum_list<T: Serialize>(
    params: &mut Vec<(String, String)>,
    key: &str,
    values: Option<&[T]>,
) -> Result<()> {
    if let Some(values) = values {
        for value in values {
            let encoded = serde_json::to_string(value)?;
            params.push((key.into(), trim_json_string(&encoded)));
        }
    }
    Ok(())
}

fn append_str_list(params: &mut Vec<(String, String)>, key: &str, values: Option<&[&str]>) {
    if let Some(values) = values {
        for value in values {
            params.push((key.into(), (*value).into()));
        }
    }
}

fn trim_json_string(s: &str) -> String {
    s.trim_matches('"').to_string()
}

fn build_query_path(path: &str, params: &[(String, String)]) -> String {
    if params.is_empty() {
        return path.to_string();
    }
    let pairs: Vec<(&str, &str)> = params
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    OvhClient::append_query(path, &pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_query_path_encodes_search() {
        let path = build_query_path(
            "/domain/name",
            &[("searchValue".into(), "example.com".into())],
        );
        assert_eq!(path, "/domain/name?searchValue=example.com");
    }

    #[test]
    fn deserializes_v2_domain() {
        let domain: V2Domain = serde_json::from_str(
            r#"{
                "id": "example.com",
                "checksum": "abc",
                "resourceStatus": "READY",
                "currentState": {
                    "name": "example.com",
                    "extension": "com",
                    "mainState": "OK",
                    "protectionState": "UNPROTECTED",
                    "suspensionState": "NOT_SUSPENDED",
                    "createdAt": "2024-01-01T00:00:00Z"
                },
                "targetSpec": {},
                "currentTasks": []
            }"#,
        )
        .unwrap();
        assert_eq!(domain.id, "example.com");
    }
}
