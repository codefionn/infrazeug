//! Request and response types for the NetBird Management API.
//!
//! Response fields are deliberately optional unless a caller must supply them
//! in a request. NetBird adds fields regularly and self-hosted releases can lag
//! behind the cloud API.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A peer in the NetBird overlay.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Peer {
    pub id: Option<String>,
    pub name: Option<String>,
    pub hostname: Option<String>,
    pub ip: Option<String>,
    pub ipv6: Option<String>,
    pub connected: Option<bool>,
    pub groups: Option<Vec<GroupMinimum>>,
    pub ssh_enabled: Option<bool>,
    pub login_expiration_enabled: Option<bool>,
    pub inactivity_expiration_enabled: Option<bool>,
    pub approval_required: Option<bool>,
    pub created_at: Option<String>,
    pub last_seen: Option<String>,
    pub user_id: Option<String>,
    pub dns_label: Option<String>,
    pub os: Option<String>,
    pub version: Option<String>,
}

/// Values accepted when updating a peer.
#[derive(Debug, Clone, Serialize)]
pub struct PeerRequest {
    pub name: String,
    pub ssh_enabled: bool,
    pub login_expiration_enabled: bool,
    pub inactivity_expiration_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<String>,
}

/// A small peer representation embedded in other responses.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PeerMinimum {
    pub id: Option<String>,
    pub name: Option<String>,
}

/// A setup key, with its key redacted by NetBird after creation.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct SetupKey {
    pub id: Option<String>,
    pub key: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub key_type: Option<String>,
    pub expires: Option<String>,
    pub valid: Option<bool>,
    pub revoked: Option<bool>,
    pub used_times: Option<u64>,
    pub usage_limit: Option<u64>,
    pub auto_groups: Option<Vec<String>>,
    pub ephemeral: Option<bool>,
    pub allow_extra_dns_labels: Option<bool>,
}

impl std::fmt::Debug for SetupKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SetupKey")
            .field("id", &self.id)
            .field("key", &self.key.as_ref().map(|_| "[redacted]"))
            .field("name", &self.name)
            .field("key_type", &self.key_type)
            .field("expires", &self.expires)
            .field("valid", &self.valid)
            .field("revoked", &self.revoked)
            .field("used_times", &self.used_times)
            .field("usage_limit", &self.usage_limit)
            .field("auto_groups", &self.auto_groups)
            .field("ephemeral", &self.ephemeral)
            .field("allow_extra_dns_labels", &self.allow_extra_dns_labels)
            .finish()
    }
}

/// A newly created setup key. The plaintext key is returned only once.
#[derive(Clone, Deserialize)]
pub struct SetupKeyClear {
    #[serde(flatten)]
    pub setup_key: SetupKey,
}

impl std::fmt::Debug for SetupKeyClear {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut key = self.setup_key.clone();
        key.key = key.key.as_ref().map(|_| "[redacted]".into());
        formatter.debug_tuple("SetupKeyClear").field(&key).finish()
    }
}

/// Values accepted when creating a setup key.
#[derive(Debug, Clone, Serialize)]
pub struct CreateSetupKeyRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub key_type: String,
    pub expires_in: u64,
    pub auto_groups: Vec<String>,
    pub usage_limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ephemeral: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_extra_dns_labels: Option<bool>,
}

/// Values accepted when updating a setup key.
#[derive(Debug, Clone, Serialize)]
pub struct SetupKeyRequest {
    pub revoked: bool,
    pub auto_groups: Vec<String>,
}

/// A group used for peers and network resources.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupMinimum {
    pub id: Option<String>,
    pub name: Option<String>,
    pub peers_count: Option<u64>,
    pub resources_count: Option<u64>,
    pub issued: Option<String>,
}

/// A NetBird group.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Group {
    #[serde(flatten)]
    pub minimum: GroupMinimum,
    pub peers: Option<Vec<PeerMinimum>>,
    pub resources: Option<Vec<Resource>>,
}

/// Values accepted when creating or updating a group.
#[derive(Debug, Clone, Serialize)]
pub struct GroupRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<Vec<Resource>>,
}

/// An identity provider configured for a NetBird account.
///
/// NetBird deliberately omits `client_secret` from this representation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityProvider {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub provider_type: Option<String>,
    pub name: Option<String>,
    pub issuer: Option<String>,
    pub client_id: Option<String>,
}

/// Values accepted when creating or replacing an identity provider.
///
/// The Management API requires the client secret for both create and update,
/// but never returns it afterwards.
#[derive(Clone, Serialize)]
pub struct IdentityProviderRequest {
    #[serde(rename = "type")]
    pub provider_type: String,
    pub name: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
}

impl std::fmt::Debug for IdentityProviderRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IdentityProviderRequest")
            .field("provider_type", &self.provider_type)
            .field("name", &self.name)
            .field("issuer", &self.issuer)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[redacted]")
            .finish()
    }
}

/// A peer or network resource referred to by a policy or group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
}

/// A policy rule port range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulePortRange {
    pub start: u16,
    pub end: u16,
}

/// A policy rule accepted in create and update requests.
#[derive(Debug, Clone, Serialize)]
pub struct PolicyRuleRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub enabled: bool,
    pub action: String,
    pub bidirectional: bool,
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_ranges: Option<Vec<RulePortRange>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorized_groups: Option<BTreeMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<String>>,
    #[serde(rename = "sourceResource", skip_serializing_if = "Option::is_none")]
    pub source_resource: Option<Resource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destinations: Option<Vec<String>>,
    #[serde(
        rename = "destinationResource",
        skip_serializing_if = "Option::is_none"
    )]
    pub destination_resource: Option<Resource>,
}

/// A policy rule returned by NetBird.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PolicyRule {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub action: Option<String>,
    pub bidirectional: Option<bool>,
    pub protocol: Option<String>,
    pub ports: Option<Vec<String>>,
    pub port_ranges: Option<Vec<RulePortRange>>,
    pub authorized_groups: Option<BTreeMap<String, Vec<String>>>,
    pub sources: Option<Vec<GroupMinimum>>,
    #[serde(rename = "sourceResource")]
    pub source_resource: Option<Resource>,
    pub destinations: Option<Vec<GroupMinimum>>,
    #[serde(rename = "destinationResource")]
    pub destination_resource: Option<Resource>,
}

/// Values accepted when creating or updating a policy.
#[derive(Debug, Clone, Serialize)]
pub struct PolicyRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub enabled: bool,
    pub rules: Vec<PolicyRuleRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_posture_checks: Option<Vec<String>>,
}

/// A NetBird policy.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Policy {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,
    pub rules: Option<Vec<PolicyRule>>,
    pub source_posture_checks: Option<Vec<String>>,
}

/// Values accepted when creating or updating a route.
#[derive(Debug, Clone, Serialize)]
pub struct RouteRequest {
    pub description: String,
    pub network_id: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_groups: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domains: Option<Vec<String>>,
    pub metric: u32,
    pub masquerade: bool,
    pub groups: Vec<String>,
    pub keep_route: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_control_groups: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_auto_apply: Option<bool>,
}

/// A route returned by NetBird.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Route {
    pub id: Option<String>,
    pub description: Option<String>,
    pub network_id: Option<String>,
    pub network_type: Option<String>,
    pub enabled: Option<bool>,
    pub peer: Option<String>,
    pub peer_groups: Option<Vec<String>>,
    pub network: Option<String>,
    pub domains: Option<Vec<String>>,
    pub metric: Option<u32>,
    pub masquerade: Option<bool>,
    pub groups: Option<Vec<String>>,
    pub keep_route: Option<bool>,
    pub access_control_groups: Option<Vec<String>>,
    pub skip_auto_apply: Option<bool>,
}

/// Values accepted when creating or updating a network.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A NetBird network.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Network {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub routers: Option<Vec<String>>,
    pub resources: Option<Vec<String>>,
    pub policies: Option<Vec<String>>,
    pub routing_peers_count: Option<u64>,
}

/// Values accepted when creating or updating a network resource.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkResourceRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub address: String,
    pub enabled: bool,
    pub groups: Vec<String>,
}

/// A resource inside a NetBird network.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NetworkResource {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub resource_type: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub enabled: Option<bool>,
    pub groups: Option<Vec<GroupMinimum>>,
}

/// Values accepted when creating or updating a network router.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkRouterRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_groups: Option<Vec<String>>,
    pub metric: u32,
    pub masquerade: bool,
    pub enabled: bool,
}

/// A router attached to a NetBird network.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NetworkRouter {
    pub id: Option<String>,
    pub peer: Option<String>,
    pub peer_groups: Option<Vec<String>>,
    pub metric: Option<u32>,
    pub masquerade: Option<bool>,
    pub enabled: Option<bool>,
}

/// A custom domain registered for a BYOP reverse-proxy cluster.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReverseProxyDomain {
    pub id: Option<String>,
    pub domain: Option<String>,
    #[serde(rename = "type")]
    pub domain_type: Option<String>,
    pub validated: Option<bool>,
    pub target_cluster: Option<String>,
    pub require_subdomain: Option<bool>,
    pub supports_crowdsec: Option<bool>,
    pub supports_custom_ports: Option<bool>,
    pub supports_private: Option<bool>,
}

/// Values accepted when registering a custom reverse-proxy domain.
#[derive(Debug, Clone, Serialize)]
pub struct ReverseProxyDomainRequest {
    pub domain: String,
    pub target_cluster: String,
}

/// A connected reverse-proxy cluster. Account-owned clusters are created when
/// a BYOP instance registers with its one-time proxy token.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReverseProxyCluster {
    pub id: Option<String>,
    pub address: Option<String>,
    pub connected_proxies: Option<u64>,
    pub online: Option<bool>,
    #[serde(rename = "type")]
    pub cluster_type: Option<String>,
}

/// Metadata for an account-scoped BYOP reverse-proxy token.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReverseProxyToken {
    pub id: Option<String>,
    pub name: Option<String>,
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
    pub last_used: Option<String>,
    pub revoked: Option<bool>,
}

/// Values accepted when minting a reverse-proxy token.
#[derive(Debug, Clone, Serialize)]
pub struct CreateReverseProxyTokenRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
}

/// The one-time response from creating a reverse-proxy token.
#[derive(Clone, Deserialize)]
pub struct ReverseProxyTokenCreated {
    #[serde(flatten)]
    pub token: ReverseProxyToken,
    pub plain_token: String,
}

impl std::fmt::Debug for ReverseProxyTokenCreated {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReverseProxyTokenCreated")
            .field("token", &self.token)
            .field("plain_token", &"[redacted]")
            .finish()
    }
}

/// A backend exposed through a NetBird reverse-proxy service.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReverseProxyServiceTarget {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub port: u16,
    pub protocol: String,
    pub target_id: String,
    pub target_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<ReverseProxyServiceTargetOptions>,
}

/// Optional connection settings for a reverse-proxy target.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReverseProxyServiceTargetOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_headers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_upstream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_rewrite: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_protocol: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_idle_timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_tls_verify: Option<bool>,
}

/// Reverse-proxy authentication settings. Empty settings mean public access.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReverseProxyServiceAuth {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearer_auth: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_auths: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_auth: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_auth: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_auth: Option<serde_json::Value>,
}

/// Values accepted when creating or updating a reverse-proxy service.
#[derive(Debug, Clone, Serialize)]
pub struct ReverseProxyServiceRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_groups: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<ReverseProxyServiceAuth>,
    pub domain: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pass_host_header: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewrite_redirects: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<ReverseProxyServiceTarget>>,
}

/// A reverse-proxy service returned by the Management API.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ReverseProxyService {
    pub id: Option<String>,
    pub name: Option<String>,
    pub domain: Option<String>,
    pub enabled: Option<bool>,
    pub targets: Option<Vec<ReverseProxyServiceTarget>>,
    pub access_groups: Option<Vec<String>>,
    pub auth: Option<ReverseProxyServiceAuth>,
    pub listen_port: Option<u16>,
    pub mode: Option<String>,
    pub pass_host_header: Option<bool>,
    pub private: Option<bool>,
    pub rewrite_redirects: Option<bool>,
    pub proxy_cluster: Option<String>,
    pub terminated: Option<bool>,
}

/// A DNS resolver in a nameserver group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nameserver {
    pub ip: String,
    pub ns_type: String,
    pub port: u16,
}

/// Values accepted when creating or updating a nameserver group.
#[derive(Debug, Clone, Serialize)]
pub struct NameserverGroupRequest {
    pub name: String,
    pub description: String,
    pub nameservers: Vec<Nameserver>,
    pub enabled: bool,
    pub groups: Vec<String>,
    pub primary: bool,
    pub domains: Vec<String>,
    pub search_domains_enabled: bool,
}

/// A DNS nameserver group.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NameserverGroup {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub nameservers: Option<Vec<Nameserver>>,
    pub enabled: Option<bool>,
    pub groups: Option<Vec<String>>,
    pub primary: Option<bool>,
    pub domains: Option<Vec<String>>,
    pub search_domains_enabled: Option<bool>,
}

/// DNS settings for an account.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DnsSettings {
    pub disabled_management_groups: Vec<String>,
}

/// An account visible to the authenticated user.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Account {
    pub id: Option<String>,
    pub domain: Option<String>,
    pub domain_category: Option<String>,
    pub created_at: Option<String>,
    pub created_by: Option<String>,
    pub settings: Option<serde_json::Value>,
    pub onboarding: Option<serde_json::Value>,
}

impl Account {
    /// Decode the account settings while preserving the original JSON in [`Self::settings`].
    pub fn account_settings(&self) -> serde_json::Result<Option<AccountSettings>> {
        self.settings
            .clone()
            .map(serde_json::from_value)
            .transpose()
    }

    /// Decode the onboarding state while preserving the original JSON in [`Self::onboarding`].
    pub fn onboarding_state(&self) -> serde_json::Result<Option<AccountOnboarding>> {
        self.onboarding
            .clone()
            .map(serde_json::from_value)
            .transpose()
    }
}

/// Account settings returned by NetBird.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AccountSettings {
    pub peer_login_expiration_enabled: Option<bool>,
    pub peer_login_expiration: Option<u64>,
    pub peer_inactivity_expiration_enabled: Option<bool>,
    pub peer_inactivity_expiration: Option<u64>,
    pub regular_users_view_blocked: Option<bool>,
    pub groups_propagation_enabled: Option<bool>,
    pub jwt_groups_enabled: Option<bool>,
    pub jwt_groups_claim_name: Option<String>,
    pub jwt_allow_groups: Option<Vec<String>>,
    pub routing_peer_dns_resolution_enabled: Option<bool>,
    pub dns_domain: Option<String>,
    pub network_range: Option<String>,
    pub network_range_v6: Option<String>,
    pub peer_expose_enabled: Option<bool>,
    pub peer_expose_groups: Option<Vec<String>>,
    pub extra: Option<AccountExtraSettings>,
    pub lazy_connection_enabled: Option<bool>,
    pub auto_update_version: Option<String>,
    pub auto_update_always: Option<bool>,
    pub metrics_push_enabled: Option<bool>,
    pub agent_network_only: Option<bool>,
    pub dashboard_features: Option<AccountDashboardFeatures>,
    pub embedded_idp_enabled: Option<bool>,
    pub local_auth_disabled: Option<bool>,
    pub local_mfa_enabled: Option<bool>,
    pub ipv6_enabled_groups: Option<Vec<String>>,
}

/// Account settings accepted by the account update endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct AccountSettingsRequest {
    pub peer_login_expiration_enabled: bool,
    pub peer_login_expiration: u64,
    pub peer_inactivity_expiration_enabled: bool,
    pub peer_inactivity_expiration: u64,
    pub regular_users_view_blocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups_propagation_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jwt_groups_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jwt_groups_claim_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jwt_allow_groups: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing_peer_dns_resolution_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns_domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_range_v6: Option<String>,
    pub peer_expose_enabled: bool,
    pub peer_expose_groups: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<AccountExtraSettingsRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lazy_connection_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_update_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_update_always: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics_push_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_network_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dashboard_features: Option<AccountDashboardFeatures>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_mfa_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6_enabled_groups: Option<Vec<String>>,
}

/// Optional dashboard visibility overrides.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountDashboardFeatures {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_network: Option<bool>,
}

/// Additional account-wide controls.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AccountExtraSettings {
    pub peer_approval_enabled: Option<bool>,
    pub user_approval_required: Option<bool>,
    pub network_traffic_logs_enabled: Option<bool>,
    pub network_traffic_logs_groups: Option<Vec<String>>,
    pub network_traffic_packet_counter_enabled: Option<bool>,
}

/// Additional account-wide controls accepted by an account update.
#[derive(Debug, Clone, Serialize)]
pub struct AccountExtraSettingsRequest {
    pub peer_approval_enabled: bool,
    pub user_approval_required: bool,
    pub network_traffic_logs_enabled: bool,
    pub network_traffic_logs_groups: Vec<String>,
    pub network_traffic_packet_counter_enabled: bool,
}

/// Account onboarding state.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AccountOnboarding {
    pub signup_form_pending: Option<bool>,
    pub onboarding_flow_pending: Option<bool>,
}

/// Account onboarding state accepted by an account update.
#[derive(Debug, Clone, Serialize)]
pub struct AccountOnboardingRequest {
    pub signup_form_pending: bool,
    pub onboarding_flow_pending: bool,
}

/// Values accepted when updating an account.
#[derive(Debug, Clone, Serialize)]
pub struct AccountRequest {
    pub settings: AccountSettingsRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onboarding: Option<AccountOnboardingRequest>,
}

/// Values accepted when creating or updating a posture check.
#[derive(Debug, Clone, Serialize)]
pub struct PostureCheckRequest {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks: Option<Checks>,
}

/// A posture check returned by NetBird.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PostureCheck {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub checks: Option<Checks>,
}

/// The check definitions attached to a posture check.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Checks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nb_version_check: Option<NbVersionCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version_check: Option<OsVersionCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo_location_check: Option<GeoLocationCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_network_range_check: Option<PeerNetworkRangeCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_check: Option<ProcessCheck>,
}

/// A minimum NetBird client version requirement.
pub type NbVersionCheck = MinVersionCheck;

/// Per-operating-system minimum version requirements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OsVersionCheck {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub android: Option<MinVersionCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub darwin: Option<MinVersionCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ios: Option<MinVersionCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux: Option<MinKernelVersionCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows: Option<MinKernelVersionCheck>,
}

/// A minimum application or operating-system version requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinVersionCheck {
    pub min_version: String,
}

/// A minimum kernel-version requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinKernelVersionCheck {
    pub min_kernel_version: String,
}

/// An allow or deny rule for peer geolocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoLocationCheck {
    pub locations: Vec<Location>,
    pub action: String,
}

/// An allow or deny rule for a peer's network addresses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerNetworkRangeCheck {
    pub ranges: Vec<String>,
    pub action: String,
}

/// A process-presence requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessCheck {
    pub processes: Vec<Process>,
}

/// Platform-specific executable paths for a process posture check.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Process {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows_path: Option<String>,
}

/// A country and, optionally, a city used by a geolocation posture check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub country_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city_name: Option<String>,
}

/// A country returned by the locations API.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Country {
    pub country_name: Option<String>,
    pub country_code: Option<String>,
}

/// A city returned by the locations API.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct City {
    pub geoname_id: Option<u64>,
    pub city_name: Option<String>,
}

/// Values accepted when creating or replacing a custom DNS zone.
#[derive(Debug, Clone, Serialize)]
pub struct DnsZoneRequest {
    pub name: String,
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub enable_search_domain: bool,
    pub distribution_groups: Vec<String>,
}

/// A custom DNS zone.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DnsZone {
    pub id: Option<String>,
    pub name: Option<String>,
    pub domain: Option<String>,
    pub enabled: Option<bool>,
    pub enable_search_domain: Option<bool>,
    pub distribution_groups: Option<Vec<String>>,
    pub records: Option<Vec<DnsRecord>>,
}

/// The record types accepted by NetBird custom DNS zones.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DnsRecordType {
    #[serde(rename = "A")]
    A,
    #[serde(rename = "AAAA")]
    Aaaa,
    #[serde(rename = "CNAME")]
    Cname,
}

/// Values accepted when creating or replacing a DNS record.
#[derive(Debug, Clone, Serialize)]
pub struct DnsRecordRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: DnsRecordType,
    pub content: String,
    pub ttl: u64,
}

/// A DNS record in a custom zone.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DnsRecord {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub record_type: Option<DnsRecordType>,
    pub content: Option<String>,
    pub ttl: Option<u64>,
}

/// A peer that the selected peer can reach through the NetBird network.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AccessiblePeer {
    pub id: Option<String>,
    pub name: Option<String>,
    pub ip: Option<String>,
    pub ipv6: Option<String>,
    pub dns_label: Option<String>,
    pub user_id: Option<String>,
    pub os: Option<String>,
    pub country_code: Option<String>,
    pub city_name: Option<String>,
    pub geoname_id: Option<u64>,
    pub connected: Option<bool>,
    pub last_seen: Option<String>,
}

/// A user account returned by NetBird.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub email: Option<String>,
    pub name: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub auto_groups: Option<Vec<String>>,
    pub is_blocked: Option<bool>,
    pub is_service_user: Option<bool>,
}

/// Values accepted when creating a user or service user.
#[derive(Debug, Clone, Serialize)]
pub struct UserCreateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub role: String,
    pub auto_groups: Vec<String>,
    pub is_service_user: bool,
}

/// Values accepted when updating a user.
#[derive(Debug, Clone, Serialize)]
pub struct UserRequest {
    pub role: String,
    pub auto_groups: Vec<String>,
    pub is_blocked: bool,
}

/// A personal access token. The plaintext is only present in a creation response.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PersonalAccessToken {
    pub id: Option<String>,
    pub name: Option<String>,
    pub expiration_date: Option<String>,
    pub created_by: Option<String>,
    pub created_at: Option<String>,
    pub last_used: Option<String>,
}

/// Values accepted when creating a personal access token.
#[derive(Debug, Clone, Serialize)]
pub struct PersonalAccessTokenRequest {
    pub name: String,
    pub expires_in: u32,
}

/// A one-time response that includes the new token plaintext.
#[derive(Clone, Deserialize)]
pub struct PersonalAccessTokenGenerated {
    pub plain_token: String,
    pub personal_access_token: PersonalAccessToken,
}

impl std::fmt::Debug for PersonalAccessTokenGenerated {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PersonalAccessTokenGenerated")
            .field("plain_token", &"[redacted]")
            .field("personal_access_token", &self.personal_access_token)
            .finish()
    }
}

/// An audit event. NetBird's event payload varies by activity.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Event {
    pub id: Option<String>,
    pub timestamp: Option<String>,
    pub activity: Option<String>,
    pub initiator_id: Option<String>,
    pub target_id: Option<String>,
    pub meta: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_create_body_has_no_server_id() {
        let request = RouteRequest {
            description: "office".into(),
            network_id: "office-net".into(),
            enabled: true,
            peer: Some("peer-1".into()),
            peer_groups: None,
            network: Some("192.0.2.0/24".into()),
            domains: None,
            metric: 100,
            masquerade: true,
            groups: vec!["group-1".into()],
            keep_route: false,
            access_control_groups: Some(vec!["group-2".into()]),
            skip_auto_apply: Some(false),
        };
        let value = serde_json::to_value(request).unwrap();
        assert!(value.get("id").is_none());
        assert_eq!(value["network_id"], "office-net");
    }

    #[test]
    fn reverse_proxy_service_request_uses_v077_wire_names() {
        let request = ReverseProxyServiceRequest {
            access_groups: None,
            auth: None,
            domain: "git.example.test".into(),
            enabled: true,
            listen_port: None,
            mode: Some("http".into()),
            name: "git".into(),
            pass_host_header: Some(true),
            private: None,
            rewrite_redirects: Some(true),
            targets: Some(vec![ReverseProxyServiceTarget {
                enabled: true,
                host: None,
                path: Some("/".into()),
                port: 3000,
                protocol: "http".into(),
                target_id: "peer-id".into(),
                target_type: "peer".into(),
                options: None,
            }]),
        };
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["targets"][0]["target_id"], "peer-id");
        assert_eq!(value["targets"][0]["target_type"], "peer");
        assert!(value.get("id").is_none());
    }

    #[test]
    fn reverse_proxy_domain_request_uses_target_cluster() {
        let value = serde_json::to_value(ReverseProxyDomainRequest {
            domain: "example.test".into(),
            target_cluster: "proxy.example.test".into(),
        })
        .unwrap();
        assert_eq!(value["target_cluster"], "proxy.example.test");
    }

    #[test]
    fn reverse_proxy_token_creation_response_redacts_plaintext_debug() {
        let token = ReverseProxyTokenCreated {
            token: ReverseProxyToken {
                id: Some("token-id".into()),
                name: Some("byop".into()),
                ..Default::default()
            },
            plain_token: "top-secret".into(),
        };
        assert!(!format!("{token:?}").contains("top-secret"));
    }

    #[test]
    fn account_request_uses_the_management_api_settings_shape() {
        let value = serde_json::to_value(AccountRequest {
            settings: AccountSettingsRequest {
                peer_login_expiration_enabled: true,
                peer_login_expiration: 43_200,
                peer_inactivity_expiration_enabled: false,
                peer_inactivity_expiration: 86_400,
                regular_users_view_blocked: false,
                groups_propagation_enabled: None,
                jwt_groups_enabled: None,
                jwt_groups_claim_name: None,
                jwt_allow_groups: None,
                routing_peer_dns_resolution_enabled: None,
                dns_domain: None,
                network_range: None,
                network_range_v6: None,
                peer_expose_enabled: true,
                peer_expose_groups: vec!["group-1".into()],
                extra: None,
                lazy_connection_enabled: None,
                auto_update_version: None,
                auto_update_always: None,
                metrics_push_enabled: None,
                agent_network_only: None,
                dashboard_features: None,
                local_mfa_enabled: None,
                ipv6_enabled_groups: None,
            },
            onboarding: None,
        })
        .unwrap();

        assert_eq!(value["settings"]["peer_login_expiration"], 43_200);
        assert_eq!(value["settings"]["peer_expose_groups"][0], "group-1");
        assert!(value.get("onboarding").is_none());
    }

    #[test]
    fn posture_check_request_serializes_nested_check_definitions() {
        let value = serde_json::to_value(PostureCheckRequest {
            name: "managed-clients".into(),
            description: "Require a supported NetBird client".into(),
            checks: Some(Checks {
                nb_version_check: Some(MinVersionCheck {
                    min_version: "0.30.0".into(),
                }),
                os_version_check: None,
                geo_location_check: None,
                peer_network_range_check: None,
                process_check: None,
            }),
        })
        .unwrap();

        assert_eq!(value["checks"]["nb_version_check"]["min_version"], "0.30.0");
        assert!(value["checks"].get("os_version_check").is_none());
    }

    #[test]
    fn dns_zone_and_record_requests_use_exact_wire_names() {
        let zone = serde_json::to_value(DnsZoneRequest {
            name: "office".into(),
            domain: "office.example.test".into(),
            enabled: None,
            enable_search_domain: true,
            distribution_groups: vec!["all".into()],
        })
        .unwrap();
        let record = serde_json::to_value(DnsRecordRequest {
            name: "git.office.example.test".into(),
            record_type: DnsRecordType::Aaaa,
            content: "2001:db8::10".into(),
            ttl: 300,
        })
        .unwrap();

        assert!(zone.get("enabled").is_none());
        assert_eq!(zone["enable_search_domain"], true);
        assert_eq!(record["type"], "AAAA");
        assert_eq!(record["ttl"], 300);
    }

    #[test]
    fn response_types_accept_partial_and_complete_api_responses() {
        let account: Account = serde_json::from_value(serde_json::json!({
            "id": "account-1",
            "settings": {
                "peer_login_expiration_enabled": true,
                "peer_login_expiration": 43200,
                "embedded_idp_enabled": false
            },
            "onboarding": {"signup_form_pending": true}
        }))
        .unwrap();
        let posture: PostureCheck = serde_json::from_value(serde_json::json!({
            "id": "check-1",
            "name": "office",
            "checks": {
                "geo_location_check": {
                    "locations": [{"country_code": "DE", "city_name": "Berlin"}],
                    "action": "allow"
                }
            }
        }))
        .unwrap();
        let peer: AccessiblePeer = serde_json::from_value(serde_json::json!({
            "id": "peer-1",
            "name": "laptop",
            "ip": "100.64.0.2",
            "connected": true
        }))
        .unwrap();
        let city: City = serde_json::from_value(serde_json::json!({
            "geoname_id": 2950159,
            "city_name": "Berlin"
        }))
        .unwrap();

        assert_eq!(
            account
                .account_settings()
                .unwrap()
                .as_ref()
                .and_then(|settings| settings.peer_login_expiration),
            Some(43_200)
        );
        assert_eq!(
            account
                .onboarding_state()
                .unwrap()
                .and_then(|state| state.signup_form_pending),
            Some(true)
        );
        assert_eq!(posture.id.as_deref(), Some("check-1"));
        assert_eq!(peer.connected, Some(true));
        assert_eq!(city.city_name.as_deref(), Some("Berlin"));
    }
}
