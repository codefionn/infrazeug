//! HTTP client and Management API operations.

use crate::auth::Auth;
use crate::error::{NetBirdError, Result};
use crate::types::*;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::{Client, Method, Response};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Default NetBird Cloud Management API host.
pub const DEFAULT_HOST: &str = "https://api.netbird.io";
const API_PATH: &str = "/api";

/// Connection configuration for a NetBird Management API client.
#[derive(Debug, Clone)]
pub struct NetBirdConfig {
    /// Management API host. Trailing slashes are stripped.
    pub host: String,
    /// A NetBird personal access token or OAuth token.
    pub auth: Auth,
}

impl NetBirdConfig {
    /// Create configuration for NetBird Cloud.
    pub fn new(auth: Auth) -> Self {
        Self {
            host: DEFAULT_HOST.into(),
            auth,
        }
    }

    /// Set a NetBird Cloud or self-hosted Management API host.
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into().trim_end_matches('/').to_string();
        self
    }
}

/// An authenticated client for the NetBird Management API.
#[derive(Clone)]
pub struct NetBirdClient {
    http: Client,
    config: NetBirdConfig,
}

impl NetBirdClient {
    /// Build a client with the supplied configuration.
    pub fn new(config: NetBirdConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    /// Use a preconfigured HTTP client, for example to set timeouts or a proxy.
    pub fn with_http_client(mut self, http: Client) -> Self {
        self.http = http;
        self
    }

    /// Return this client's configuration.
    pub fn config(&self) -> &NetBirdConfig {
        &self.config
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}{}{}", self.config.host, API_PATH, path)
    }

    fn path_segment(&self, value: &str) -> String {
        urlencoding::encode(value).into_owned()
    }

    async fn request<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&B>,
    ) -> Result<T> {
        let mut headers = HeaderMap::new();
        self.config
            .auth
            .apply(&mut headers)
            .map_err(NetBirdError::Auth)?;
        if body.is_some() {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }
        let mut request = self
            .http
            .request(method, self.api_url(path))
            .headers(headers)
            .query(query);
        if let Some(body) = body {
            request = request.json(body);
        }
        decode_response(request.send().await?).await
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T> {
        self.request::<T, serde_json::Value>(Method::GET, path, query, None)
            .await
    }

    async fn post<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.request(Method::POST, path, &[], Some(body)).await
    }

    async fn put<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.request(Method::PUT, path, &[], Some(body)).await
    }

    async fn delete(&self, path: &str) -> Result<()> {
        self.request_empty(Method::DELETE, path).await
    }

    async fn request_empty(&self, method: Method, path: &str) -> Result<()> {
        let mut headers = HeaderMap::new();
        self.config
            .auth
            .apply(&mut headers)
            .map_err(NetBirdError::Auth)?;
        decode_empty(
            self.http
                .request(method, self.api_url(path))
                .headers(headers)
                .send()
                .await?,
        )
        .await
    }

    /// List peers. The optional values filter exact name and IP matches.
    pub async fn peers(&self, name: Option<&str>, ip: Option<&str>) -> Result<Vec<Peer>> {
        let mut query = Vec::new();
        if let Some(name) = name {
            query.push(("name", name.to_owned()));
        }
        if let Some(ip) = ip {
            query.push(("ip", ip.to_owned()));
        }
        self.get("/peers", &query).await
    }

    /// Get one peer by id.
    pub async fn peer(&self, peer_id: &str) -> Result<Peer> {
        self.get(&format!("/peers/{}", self.path_segment(peer_id)), &[])
            .await
    }

    /// Update a peer's mutable settings.
    pub async fn update_peer(&self, peer_id: &str, request: &PeerRequest) -> Result<Peer> {
        self.put(&format!("/peers/{}", self.path_segment(peer_id)), request)
            .await
    }

    /// Remove a peer from the account.
    pub async fn delete_peer(&self, peer_id: &str) -> Result<()> {
        self.delete(&format!("/peers/{}", self.path_segment(peer_id)))
            .await
    }

    /// List peers that the given peer may connect to.
    pub async fn accessible_peers(&self, peer_id: &str) -> Result<Vec<AccessiblePeer>> {
        // Some self-hosted Management releases return `null` before any
        // accessible peers have been calculated. The API contract says array.
        let value: serde_json::Value = self
            .get(
                &format!("/peers/{}/accessible-peers", self.path_segment(peer_id)),
                &[],
            )
            .await?;
        decode_nullable_list(value)
    }

    /// List setup keys. Key strings are masked in normal responses.
    pub async fn setup_keys(&self) -> Result<Vec<SetupKey>> {
        self.get("/setup-keys", &[]).await
    }

    /// Get a setup key by id.
    pub async fn setup_key(&self, key_id: &str) -> Result<SetupKey> {
        self.get(&format!("/setup-keys/{}", self.path_segment(key_id)), &[])
            .await
    }

    /// Create a setup key. Its plaintext is returned once in [`SetupKeyClear`].
    pub async fn create_setup_key(&self, request: &CreateSetupKeyRequest) -> Result<SetupKeyClear> {
        self.post("/setup-keys", request).await
    }

    /// Update a setup key's revocation and automatic groups.
    pub async fn update_setup_key(
        &self,
        key_id: &str,
        request: &SetupKeyRequest,
    ) -> Result<SetupKey> {
        self.put(
            &format!("/setup-keys/{}", self.path_segment(key_id)),
            request,
        )
        .await
    }

    /// Delete a setup key.
    pub async fn delete_setup_key(&self, key_id: &str) -> Result<()> {
        self.delete(&format!("/setup-keys/{}", self.path_segment(key_id)))
            .await
    }

    /// List groups, optionally by exact name.
    pub async fn groups(&self, name: Option<&str>) -> Result<Vec<Group>> {
        let query = name
            .map(|name| vec![("name", name.to_owned())])
            .unwrap_or_default();
        match self.get("/groups", &query).await {
            Ok(groups) => Ok(groups),
            // NetBird v0.77.1 returns 404 instead of an empty list when the
            // optional exact-name filter has no match. A collection lookup for
            // a missing group is still an empty collection.
            Err(NetBirdError::Api { status: 404, .. }) if name.is_some() => Ok(Vec::new()),
            Err(error) => Err(error),
        }
    }

    /// Get a group by id.
    pub async fn group(&self, group_id: &str) -> Result<Group> {
        self.get(&format!("/groups/{}", self.path_segment(group_id)), &[])
            .await
    }

    /// Create a group.
    pub async fn create_group(&self, request: &GroupRequest) -> Result<Group> {
        self.post("/groups", request).await
    }

    /// Update a group.
    pub async fn update_group(&self, group_id: &str, request: &GroupRequest) -> Result<Group> {
        self.put(&format!("/groups/{}", self.path_segment(group_id)), request)
            .await
    }

    /// Delete a group.
    pub async fn delete_group(&self, group_id: &str) -> Result<()> {
        self.delete(&format!("/groups/{}", self.path_segment(group_id)))
            .await
    }

    /// List identity providers configured for the account. Client secrets are
    /// intentionally absent from the response.
    pub async fn identity_providers(&self) -> Result<Vec<IdentityProvider>> {
        self.get("/identity-providers", &[]).await
    }

    /// Create an identity provider.
    pub async fn create_identity_provider(
        &self,
        request: &IdentityProviderRequest,
    ) -> Result<IdentityProvider> {
        self.post("/identity-providers", request).await
    }

    /// Replace an identity provider configuration.
    pub async fn update_identity_provider(
        &self,
        provider_id: &str,
        request: &IdentityProviderRequest,
    ) -> Result<IdentityProvider> {
        self.put(
            &format!("/identity-providers/{}", self.path_segment(provider_id)),
            request,
        )
        .await
    }

    /// Delete an identity provider.
    pub async fn delete_identity_provider(&self, provider_id: &str) -> Result<()> {
        self.delete(&format!(
            "/identity-providers/{}",
            self.path_segment(provider_id)
        ))
        .await
    }

    /// List policies.
    pub async fn policies(&self) -> Result<Vec<Policy>> {
        self.get("/policies", &[]).await
    }

    /// Get a policy by id.
    pub async fn policy(&self, policy_id: &str) -> Result<Policy> {
        self.get(&format!("/policies/{}", self.path_segment(policy_id)), &[])
            .await
    }

    /// Create a policy.
    pub async fn create_policy(&self, request: &PolicyRequest) -> Result<Policy> {
        self.post("/policies", request).await
    }

    /// Replace a policy.
    pub async fn update_policy(&self, policy_id: &str, request: &PolicyRequest) -> Result<Policy> {
        self.put(
            &format!("/policies/{}", self.path_segment(policy_id)),
            request,
        )
        .await
    }

    /// Delete a policy.
    pub async fn delete_policy(&self, policy_id: &str) -> Result<()> {
        self.delete(&format!("/policies/{}", self.path_segment(policy_id)))
            .await
    }

    /// List routes.
    pub async fn routes(&self) -> Result<Vec<Route>> {
        self.get("/routes", &[]).await
    }

    /// Get a route by id.
    pub async fn route(&self, route_id: &str) -> Result<Route> {
        self.get(&format!("/routes/{}", self.path_segment(route_id)), &[])
            .await
    }

    /// Create a route.
    pub async fn create_route(&self, request: &RouteRequest) -> Result<Route> {
        self.post("/routes", request).await
    }

    /// Replace a route.
    pub async fn update_route(&self, route_id: &str, request: &RouteRequest) -> Result<Route> {
        self.put(&format!("/routes/{}", self.path_segment(route_id)), request)
            .await
    }

    /// Delete a route.
    pub async fn delete_route(&self, route_id: &str) -> Result<()> {
        self.delete(&format!("/routes/{}", self.path_segment(route_id)))
            .await
    }

    /// List networks.
    pub async fn networks(&self) -> Result<Vec<Network>> {
        self.get("/networks", &[]).await
    }

    /// Get a network by id.
    pub async fn network(&self, network_id: &str) -> Result<Network> {
        self.get(&format!("/networks/{}", self.path_segment(network_id)), &[])
            .await
    }

    /// Create a network.
    pub async fn create_network(&self, request: &NetworkRequest) -> Result<Network> {
        self.post("/networks", request).await
    }

    /// Replace a network.
    pub async fn update_network(
        &self,
        network_id: &str,
        request: &NetworkRequest,
    ) -> Result<Network> {
        self.put(
            &format!("/networks/{}", self.path_segment(network_id)),
            request,
        )
        .await
    }

    /// Delete a network.
    pub async fn delete_network(&self, network_id: &str) -> Result<()> {
        self.delete(&format!("/networks/{}", self.path_segment(network_id)))
            .await
    }

    /// List a network's resources.
    pub async fn network_resources(&self, network_id: &str) -> Result<Vec<NetworkResource>> {
        // NetBird v0.77.1 returns JSON `null` instead of `[]` for an empty
        // resource collection. Treat that historical representation as empty.
        let value: serde_json::Value = self
            .get(
                &format!("/networks/{}/resources", self.path_segment(network_id)),
                &[],
            )
            .await?;
        decode_nullable_list(value)
    }

    /// Get a resource in a network.
    pub async fn network_resource(
        &self,
        network_id: &str,
        resource_id: &str,
    ) -> Result<NetworkResource> {
        self.get(
            &format!(
                "/networks/{}/resources/{}",
                self.path_segment(network_id),
                self.path_segment(resource_id)
            ),
            &[],
        )
        .await
    }

    /// Create a resource in a network.
    pub async fn create_network_resource(
        &self,
        network_id: &str,
        request: &NetworkResourceRequest,
    ) -> Result<NetworkResource> {
        self.post(
            &format!("/networks/{}/resources", self.path_segment(network_id)),
            request,
        )
        .await
    }

    /// Replace a resource in a network.
    pub async fn update_network_resource(
        &self,
        network_id: &str,
        resource_id: &str,
        request: &NetworkResourceRequest,
    ) -> Result<NetworkResource> {
        self.put(
            &format!(
                "/networks/{}/resources/{}",
                self.path_segment(network_id),
                self.path_segment(resource_id)
            ),
            request,
        )
        .await
    }

    /// Delete a resource from a network.
    pub async fn delete_network_resource(&self, network_id: &str, resource_id: &str) -> Result<()> {
        self.delete(&format!(
            "/networks/{}/resources/{}",
            self.path_segment(network_id),
            self.path_segment(resource_id)
        ))
        .await
    }

    /// List all routers in a network.
    pub async fn network_routers(&self, network_id: &str) -> Result<Vec<NetworkRouter>> {
        self.get(
            &format!("/networks/{}/routers", self.path_segment(network_id)),
            &[],
        )
        .await
    }

    /// List all routers across networks.
    pub async fn all_network_routers(&self) -> Result<Vec<NetworkRouter>> {
        self.get("/networks/routers", &[]).await
    }

    /// Get a network router.
    pub async fn network_router(&self, network_id: &str, router_id: &str) -> Result<NetworkRouter> {
        self.get(
            &format!(
                "/networks/{}/routers/{}",
                self.path_segment(network_id),
                self.path_segment(router_id)
            ),
            &[],
        )
        .await
    }

    /// Create a router in a network.
    pub async fn create_network_router(
        &self,
        network_id: &str,
        request: &NetworkRouterRequest,
    ) -> Result<NetworkRouter> {
        self.post(
            &format!("/networks/{}/routers", self.path_segment(network_id)),
            request,
        )
        .await
    }

    /// Replace a network router.
    pub async fn update_network_router(
        &self,
        network_id: &str,
        router_id: &str,
        request: &NetworkRouterRequest,
    ) -> Result<NetworkRouter> {
        self.put(
            &format!(
                "/networks/{}/routers/{}",
                self.path_segment(network_id),
                self.path_segment(router_id)
            ),
            request,
        )
        .await
    }

    /// Delete a network router.
    pub async fn delete_network_router(&self, network_id: &str, router_id: &str) -> Result<()> {
        self.delete(&format!(
            "/networks/{}/routers/{}",
            self.path_segment(network_id),
            self.path_segment(router_id)
        ))
        .await
    }

    /// List connected reverse-proxy clusters. A BYOP instance appears here
    /// after it has registered with the Management server.
    pub async fn reverse_proxy_clusters(&self) -> Result<Vec<ReverseProxyCluster>> {
        // NetBird v0.77.1 returns JSON `null` when the account has never had a
        // reverse-proxy cluster. Its documented collection shape is an array.
        let value: serde_json::Value = self.get("/reverse-proxies/clusters", &[]).await?;
        decode_nullable_list(value)
    }

    /// List account-scoped reverse-proxy tokens. Plaintext is never returned.
    pub async fn reverse_proxy_tokens(&self) -> Result<Vec<ReverseProxyToken>> {
        self.get("/reverse-proxies/proxy-tokens", &[]).await
    }

    /// Mint a reverse-proxy token. Its plaintext is returned once only.
    pub async fn create_reverse_proxy_token(
        &self,
        request: &CreateReverseProxyTokenRequest,
    ) -> Result<ReverseProxyTokenCreated> {
        self.post("/reverse-proxies/proxy-tokens", request).await
    }

    /// Revoke a reverse-proxy token.
    pub async fn delete_reverse_proxy_token(&self, token_id: &str) -> Result<()> {
        self.delete(&format!(
            "/reverse-proxies/proxy-tokens/{}",
            self.path_segment(token_id)
        ))
        .await
    }

    /// List the reverse-proxy domains available to this account.
    pub async fn reverse_proxy_domains(&self) -> Result<Vec<ReverseProxyDomain>> {
        self.get("/reverse-proxies/domains", &[]).await
    }

    /// Register a custom domain against an already connected BYOP cluster.
    pub async fn create_reverse_proxy_domain(
        &self,
        request: &ReverseProxyDomainRequest,
    ) -> Result<ReverseProxyDomain> {
        self.post("/reverse-proxies/domains", request).await
    }

    /// Ask NetBird to validate DNS ownership of a custom reverse-proxy domain.
    pub async fn validate_reverse_proxy_domain(&self, domain_id: &str) -> Result<()> {
        self.request_empty(
            Method::GET,
            &format!(
                "/reverse-proxies/domains/{}/validate",
                self.path_segment(domain_id)
            ),
        )
        .await
    }

    /// Delete a custom reverse-proxy domain.
    pub async fn delete_reverse_proxy_domain(&self, domain_id: &str) -> Result<()> {
        self.delete(&format!(
            "/reverse-proxies/domains/{}",
            self.path_segment(domain_id)
        ))
        .await
    }

    /// List reverse-proxy services.
    pub async fn reverse_proxy_services(&self) -> Result<Vec<ReverseProxyService>> {
        self.get("/reverse-proxies/services", &[]).await
    }

    /// Get one reverse-proxy service by server-generated id.
    pub async fn reverse_proxy_service(&self, service_id: &str) -> Result<ReverseProxyService> {
        self.get(
            &format!(
                "/reverse-proxies/services/{}",
                self.path_segment(service_id)
            ),
            &[],
        )
        .await
    }

    /// Create a reverse-proxy service.
    pub async fn create_reverse_proxy_service(
        &self,
        request: &ReverseProxyServiceRequest,
    ) -> Result<ReverseProxyService> {
        self.post("/reverse-proxies/services", request).await
    }

    /// Replace a reverse-proxy service.
    pub async fn update_reverse_proxy_service(
        &self,
        service_id: &str,
        request: &ReverseProxyServiceRequest,
    ) -> Result<ReverseProxyService> {
        self.put(
            &format!(
                "/reverse-proxies/services/{}",
                self.path_segment(service_id)
            ),
            request,
        )
        .await
    }

    /// Delete a reverse-proxy service.
    pub async fn delete_reverse_proxy_service(&self, service_id: &str) -> Result<()> {
        self.delete(&format!(
            "/reverse-proxies/services/{}",
            self.path_segment(service_id)
        ))
        .await
    }

    /// List DNS nameserver groups.
    pub async fn nameserver_groups(&self) -> Result<Vec<NameserverGroup>> {
        // NetBird v0.77.1 returns JSON `null` before the account has a DNS
        // nameserver group. Its collection response is otherwise an array.
        let value: serde_json::Value = self.get("/dns/nameservers", &[]).await?;
        decode_nullable_list(value)
    }

    /// Get a DNS nameserver group.
    pub async fn nameserver_group(&self, group_id: &str) -> Result<NameserverGroup> {
        self.get(
            &format!("/dns/nameservers/{}", self.path_segment(group_id)),
            &[],
        )
        .await
    }

    /// Create a DNS nameserver group.
    pub async fn create_nameserver_group(
        &self,
        request: &NameserverGroupRequest,
    ) -> Result<NameserverGroup> {
        self.post("/dns/nameservers", request).await
    }

    /// Replace a DNS nameserver group.
    pub async fn update_nameserver_group(
        &self,
        group_id: &str,
        request: &NameserverGroupRequest,
    ) -> Result<NameserverGroup> {
        self.put(
            &format!("/dns/nameservers/{}", self.path_segment(group_id)),
            request,
        )
        .await
    }

    /// Delete a DNS nameserver group.
    pub async fn delete_nameserver_group(&self, group_id: &str) -> Result<()> {
        self.delete(&format!("/dns/nameservers/{}", self.path_segment(group_id)))
            .await
    }

    /// Read account DNS settings. Both raw and `{ "items": ... }` responses are accepted.
    pub async fn dns_settings(&self) -> Result<DnsSettings> {
        let value: serde_json::Value = self.get("/dns/settings", &[]).await?;
        decode_dns_settings_value(value)
    }

    /// Replace account DNS settings.
    pub async fn update_dns_settings(&self, request: &DnsSettings) -> Result<DnsSettings> {
        self.put("/dns/settings", request).await
    }

    /// List custom DNS zones.
    pub async fn dns_zones(&self) -> Result<Vec<DnsZone>> {
        let value: serde_json::Value = self.get("/dns/zones", &[]).await?;
        decode_nullable_list(value)
    }

    /// Get a custom DNS zone.
    pub async fn dns_zone(&self, zone_id: &str) -> Result<DnsZone> {
        self.get(&format!("/dns/zones/{}", self.path_segment(zone_id)), &[])
            .await
    }

    /// Create a custom DNS zone.
    pub async fn create_dns_zone(&self, request: &DnsZoneRequest) -> Result<DnsZone> {
        self.post("/dns/zones", request).await
    }

    /// Replace a custom DNS zone.
    pub async fn update_dns_zone(
        &self,
        zone_id: &str,
        request: &DnsZoneRequest,
    ) -> Result<DnsZone> {
        self.put(
            &format!("/dns/zones/{}", self.path_segment(zone_id)),
            request,
        )
        .await
    }

    /// Delete a custom DNS zone and its records.
    pub async fn delete_dns_zone(&self, zone_id: &str) -> Result<()> {
        self.delete(&format!("/dns/zones/{}", self.path_segment(zone_id)))
            .await
    }

    /// List records in a custom DNS zone.
    pub async fn dns_records(&self, zone_id: &str) -> Result<Vec<DnsRecord>> {
        let value: serde_json::Value = self
            .get(
                &format!("/dns/zones/{}/records", self.path_segment(zone_id)),
                &[],
            )
            .await?;
        decode_nullable_list(value)
    }

    /// Get a DNS record in a custom DNS zone.
    pub async fn dns_record(&self, zone_id: &str, record_id: &str) -> Result<DnsRecord> {
        self.get(
            &format!(
                "/dns/zones/{}/records/{}",
                self.path_segment(zone_id),
                self.path_segment(record_id)
            ),
            &[],
        )
        .await
    }

    /// Create a DNS record in a custom DNS zone.
    pub async fn create_dns_record(
        &self,
        zone_id: &str,
        request: &DnsRecordRequest,
    ) -> Result<DnsRecord> {
        self.post(
            &format!("/dns/zones/{}/records", self.path_segment(zone_id)),
            request,
        )
        .await
    }

    /// Replace a DNS record in a custom DNS zone.
    pub async fn update_dns_record(
        &self,
        zone_id: &str,
        record_id: &str,
        request: &DnsRecordRequest,
    ) -> Result<DnsRecord> {
        self.put(
            &format!(
                "/dns/zones/{}/records/{}",
                self.path_segment(zone_id),
                self.path_segment(record_id)
            ),
            request,
        )
        .await
    }

    /// Delete a DNS record from a custom DNS zone.
    pub async fn delete_dns_record(&self, zone_id: &str, record_id: &str) -> Result<()> {
        self.delete(&format!(
            "/dns/zones/{}/records/{}",
            self.path_segment(zone_id),
            self.path_segment(record_id)
        ))
        .await
    }

    /// List accounts visible to the authenticated user.
    pub async fn accounts(&self) -> Result<Vec<Account>> {
        self.get("/accounts", &[]).await
    }

    /// Replace an account's settings and optional onboarding state.
    pub async fn update_account(
        &self,
        account_id: &str,
        request: &AccountRequest,
    ) -> Result<Account> {
        self.put(
            &format!("/accounts/{}", self.path_segment(account_id)),
            request,
        )
        .await
    }

    /// Delete an account and all resources it owns.
    pub async fn delete_account(&self, account_id: &str) -> Result<()> {
        self.delete(&format!("/accounts/{}", self.path_segment(account_id)))
            .await
    }

    /// List posture checks.
    pub async fn posture_checks(&self) -> Result<Vec<PostureCheck>> {
        let value: serde_json::Value = self.get("/posture-checks", &[]).await?;
        decode_nullable_list(value)
    }

    /// Get a posture check by id.
    pub async fn posture_check(&self, posture_check_id: &str) -> Result<PostureCheck> {
        self.get(
            &format!("/posture-checks/{}", self.path_segment(posture_check_id)),
            &[],
        )
        .await
    }

    /// Create a posture check.
    pub async fn create_posture_check(
        &self,
        request: &PostureCheckRequest,
    ) -> Result<PostureCheck> {
        self.post("/posture-checks", request).await
    }

    /// Replace a posture check.
    pub async fn update_posture_check(
        &self,
        posture_check_id: &str,
        request: &PostureCheckRequest,
    ) -> Result<PostureCheck> {
        self.put(
            &format!("/posture-checks/{}", self.path_segment(posture_check_id)),
            request,
        )
        .await
    }

    /// Delete a posture check.
    pub async fn delete_posture_check(&self, posture_check_id: &str) -> Result<()> {
        self.delete(&format!(
            "/posture-checks/{}",
            self.path_segment(posture_check_id)
        ))
        .await
    }

    /// List countries available for geolocation posture checks.
    pub async fn countries(&self) -> Result<Vec<Country>> {
        // The OpenAPI response currently says `string[]`, but NetBird's
        // geolocations handler and its tests write `Country[]`.
        let value: serde_json::Value = self.get("/locations/countries", &[]).await?;
        decode_countries(value)
    }

    /// List cities available for a country code.
    pub async fn cities(&self, country: &str) -> Result<Vec<City>> {
        // The OpenAPI response currently names one `City`, but the handler
        // builds and writes a collection.
        let value: serde_json::Value = self
            .get(
                &format!("/locations/countries/{}/cities", self.path_segment(country)),
                &[],
            )
            .await?;
        decode_cities(value)
    }

    /// List regular users or service users. `None` lists both.
    pub async fn users(&self, service_user: Option<bool>) -> Result<Vec<User>> {
        let query = service_user
            .map(|value| vec![("service_user", value.to_string())])
            .unwrap_or_default();
        self.get("/users", &query).await
    }

    /// Get the authenticated user.
    pub async fn current_user(&self) -> Result<User> {
        self.get("/users/current", &[]).await
    }

    /// Create a user or service user.
    pub async fn create_user(&self, request: &UserCreateRequest) -> Result<User> {
        self.post("/users", request).await
    }

    /// Update a user.
    pub async fn update_user(&self, user_id: &str, request: &UserRequest) -> Result<User> {
        self.put(&format!("/users/{}", self.path_segment(user_id)), request)
            .await
    }

    /// Delete a user from the NetBird account.
    pub async fn delete_user(&self, user_id: &str) -> Result<()> {
        self.delete(&format!("/users/{}", self.path_segment(user_id)))
            .await
    }

    /// List a user's personal access tokens.
    pub async fn user_tokens(&self, user_id: &str) -> Result<Vec<PersonalAccessToken>> {
        self.get(
            &format!("/users/{}/tokens", self.path_segment(user_id)),
            &[],
        )
        .await
    }

    /// Get a personal access token's metadata.
    pub async fn user_token(&self, user_id: &str, token_id: &str) -> Result<PersonalAccessToken> {
        self.get(
            &format!(
                "/users/{}/tokens/{}",
                self.path_segment(user_id),
                self.path_segment(token_id)
            ),
            &[],
        )
        .await
    }

    /// Create a personal access token. The plaintext is returned only once.
    pub async fn create_user_token(
        &self,
        user_id: &str,
        request: &PersonalAccessTokenRequest,
    ) -> Result<PersonalAccessTokenGenerated> {
        self.post(
            &format!("/users/{}/tokens", self.path_segment(user_id)),
            request,
        )
        .await
    }

    /// Delete a personal access token.
    pub async fn delete_user_token(&self, user_id: &str, token_id: &str) -> Result<()> {
        self.delete(&format!(
            "/users/{}/tokens/{}",
            self.path_segment(user_id),
            self.path_segment(token_id)
        ))
        .await
    }

    /// List audit events.
    pub async fn audit_events(&self) -> Result<Vec<Event>> {
        self.get("/events/audit", &[]).await
    }
}

fn decode_dns_settings_value(value: serde_json::Value) -> Result<DnsSettings> {
    let settings = value.get("items").cloned().unwrap_or(value);
    Ok(serde_json::from_value(settings)?)
}

fn decode_nullable_list<T: DeserializeOwned>(value: serde_json::Value) -> Result<Vec<T>> {
    if value.is_null() {
        Ok(Vec::new())
    } else {
        Ok(serde_json::from_value(value)?)
    }
}

fn decode_countries(value: serde_json::Value) -> Result<Vec<Country>> {
    if let serde_json::Value::Array(values) = &value {
        if values.iter().all(serde_json::Value::is_string) {
            return Ok(values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(|code| Country {
                    country_code: Some(code.to_owned()),
                    ..Default::default()
                })
                .collect());
        }
    }
    decode_nullable_list(value)
}

fn decode_cities(value: serde_json::Value) -> Result<Vec<City>> {
    if value.is_object() {
        Ok(vec![serde_json::from_value(value)?])
    } else {
        decode_nullable_list(value)
    }
}

async fn decode_response<T: DeserializeOwned>(response: Response) -> Result<T> {
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(api_error(status.as_u16(), body));
    }
    Ok(serde_json::from_str(&body)?)
}

async fn decode_empty(response: Response) -> Result<()> {
    let status = response.status();
    let body = response.text().await?;
    if status.is_success() {
        Ok(())
    } else {
        Err(api_error(status.as_u16(), body))
    }
}

fn api_error(status: u16, body: String) -> NetBirdError {
    let message = error_message(&body).unwrap_or_else(|| {
        if body.trim().is_empty() {
            "request failed without an error body".into()
        } else {
            body.clone()
        }
    });
    NetBirdError::Api {
        status,
        message,
        body: (!body.is_empty()).then_some(body),
    }
}

fn error_message(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    find_error_message(&value)
}

fn find_error_message(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(message) if !message.is_empty() => Some(message.clone()),
        serde_json::Value::Array(values) => values.iter().find_map(find_error_message),
        serde_json::Value::Object(values) => ["message", "error", "detail", "errors"]
            .iter()
            .find_map(|key| values.get(*key).and_then(find_error_message)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_strips_trailing_slashes() {
        let config =
            NetBirdConfig::new(Auth::oauth_token("secret")).with_host("https://netbird.example///");
        assert_eq!(config.host, "https://netbird.example");
    }

    #[test]
    fn joins_api_paths_and_encodes_ids() {
        let client = NetBirdClient::new(NetBirdConfig::new(Auth::oauth_token("secret")));
        assert_eq!(
            client.api_url("/groups"),
            "https://api.netbird.io/api/groups"
        );
        assert_eq!(client.path_segment("network/a b"), "network%2Fa%20b");
    }

    #[test]
    fn dns_settings_accept_raw_and_wrapped_responses() {
        for json in [
            r#"{"disabled_management_groups":["g1"]}"#,
            r#"{"items":{"disabled_management_groups":["g1"]}}"#,
        ] {
            let value = serde_json::from_str(json).unwrap();
            let settings = decode_dns_settings_value(value).unwrap();
            assert_eq!(settings.disabled_management_groups, vec!["g1"]);
        }
    }

    #[test]
    fn location_decoders_accept_documented_and_server_shapes() {
        let country_codes = decode_countries(serde_json::json!(["DE", "US"])).unwrap();
        let countries = decode_countries(serde_json::json!([
            {"country_name":"Germany", "country_code":"DE"}
        ]))
        .unwrap();
        let one_city = decode_cities(serde_json::json!({
            "geoname_id":2950159,
            "city_name":"Berlin"
        }))
        .unwrap();

        assert_eq!(country_codes[0].country_code.as_deref(), Some("DE"));
        assert_eq!(countries[0].country_name.as_deref(), Some("Germany"));
        assert_eq!(one_city[0].city_name.as_deref(), Some("Berlin"));
    }

    #[test]
    fn secrets_are_redacted_from_debug() {
        assert!(!format!("{:?}", Auth::personal_access_token("top-secret")).contains("top-secret"));
        let token = PersonalAccessTokenGenerated {
            plain_token: "top-secret".into(),
            personal_access_token: PersonalAccessToken::default(),
        };
        assert!(!format!("{token:?}").contains("top-secret"));
    }

    #[test]
    fn error_message_handles_netbird_shapes() {
        assert_eq!(
            error_message(r#"{"message":"denied"}"#).as_deref(),
            Some("denied")
        );
        assert_eq!(
            error_message(r#"{"errors":[{"detail":"bad input"}]}"#).as_deref(),
            Some("bad input")
        );
        assert_eq!(
            api_error(403, "plain denial".into()).to_string(),
            "netbird api error 403: plain denial"
        );
    }

    #[tokio::test]
    async fn named_group_lookup_treats_netbird_not_found_as_an_empty_list() {
        use axum::{extract::Query, http::StatusCode, routing::get, Router};
        use std::collections::HashMap;

        async fn missing_group(Query(query): Query<HashMap<String, String>>) -> StatusCode {
            assert_eq!(query.get("name").map(String::as_str), Some("missing"));
            StatusCode::NOT_FOUND
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/api/groups", get(missing_group)),
            )
            .await
            .unwrap();
        });
        let client = NetBirdClient::new(
            NetBirdConfig::new(Auth::oauth_token("test-token"))
                .with_host(format!("http://{address}")),
        );

        let groups = client.groups(Some("missing")).await.unwrap();
        assert!(groups.is_empty());

        server.abort();
    }

    #[tokio::test]
    async fn empty_reverse_proxy_cluster_list_accepts_netbird_null() {
        use axum::{http::StatusCode, routing::get, Router};

        async fn no_clusters() -> (StatusCode, &'static str) {
            (StatusCode::OK, "null")
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/api/reverse-proxies/clusters", get(no_clusters)),
            )
            .await
            .unwrap();
        });
        let client = NetBirdClient::new(
            NetBirdConfig::new(Auth::oauth_token("test-token"))
                .with_host(format!("http://{address}")),
        );

        assert!(client.reverse_proxy_clusters().await.unwrap().is_empty());
        server.abort();
    }

    #[tokio::test]
    async fn empty_network_resource_list_accepts_netbird_null() {
        use axum::{http::StatusCode, routing::get, Router};

        async fn no_resources() -> (StatusCode, &'static str) {
            (StatusCode::OK, "null")
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/api/networks/network-1/resources", get(no_resources)),
            )
            .await
            .unwrap();
        });
        let client = NetBirdClient::new(
            NetBirdConfig::new(Auth::oauth_token("test-token"))
                .with_host(format!("http://{address}")),
        );

        assert!(client
            .network_resources("network-1")
            .await
            .unwrap()
            .is_empty());
        server.abort();
    }

    #[tokio::test]
    async fn empty_nameserver_group_list_accepts_netbird_null() {
        use axum::{http::StatusCode, routing::get, Router};

        async fn no_nameserver_groups() -> (StatusCode, &'static str) {
            (StatusCode::OK, "null")
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/api/dns/nameservers", get(no_nameserver_groups)),
            )
            .await
            .unwrap();
        });
        let client = NetBirdClient::new(
            NetBirdConfig::new(Auth::oauth_token("test-token"))
                .with_host(format!("http://{address}")),
        );

        assert!(client.nameserver_groups().await.unwrap().is_empty());
        server.abort();
    }

    #[derive(Debug)]
    struct CapturedRequest {
        method: String,
        path: String,
        query: Option<String>,
        authorization: Option<String>,
        content_type: Option<String>,
        body: String,
    }

    async fn contract_handler(
        axum::extract::State(captured): axum::extract::State<
            std::sync::Arc<std::sync::Mutex<Vec<CapturedRequest>>>,
        >,
        request: axum::extract::Request,
    ) -> axum::response::Response {
        use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};

        let (parts, body) = request.into_parts();
        let bytes = to_bytes(body, 1024 * 1024).await.unwrap();
        let path = parts.uri.path().to_owned();
        let method = parts.method.to_string();
        captured.lock().unwrap().push(CapturedRequest {
            method: method.clone(),
            path: path.clone(),
            query: parts.uri.query().map(str::to_owned),
            authorization: parts
                .headers
                .get(reqwest::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            content_type: parts
                .headers
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            body: String::from_utf8(bytes.to_vec()).unwrap(),
        });

        if method == "DELETE" {
            return StatusCode::NO_CONTENT.into_response();
        }

        let response = match (method.as_str(), path.as_str()) {
            ("GET", "/api/peers/peer%2Fone%20two/accessible-peers") => {
                serde_json::json!([{"id":"reachable","name":"app","ip":"100.64.0.8"}])
            }
            ("GET", "/api/posture-checks") => serde_json::Value::Null,
            ("GET", "/api/locations/countries") => serde_json::json!([
                {"country_name":"Germany","country_code":"DE"},
                {"country_name":"United States","country_code":"US"}
            ]),
            ("GET", "/api/locations/countries/DE%2Ftest/cities") => {
                serde_json::json!([{"geoname_id":2950158,"city_name":"Berlin"}])
            }
            ("GET", "/api/users") => serde_json::json!([]),
            ("GET", _) if path.starts_with("/api/posture-checks/") => {
                serde_json::json!({"id":"posture/one","name":"minimum-version","description":"client version","checks":{}})
            }
            ("POST" | "PUT", _) if path.starts_with("/api/posture-checks") => {
                serde_json::json!({"id":"posture/one","name":"minimum-version","description":"client version","checks":{}})
            }
            ("GET", "/api/dns/zones") => serde_json::Value::Null,
            ("GET", _) if path.ends_with("/records") => serde_json::Value::Null,
            ("GET", _) if path.contains("/records/") => serde_json::json!({
                "id":"record/one", "name":"www.example.test", "type":"A", "content":"100.64.0.8", "ttl":60
            }),
            ("POST" | "PUT", _) if path.contains("/records") => serde_json::json!({
                "id":"record/one", "name":"www.example.test", "type":"A", "content":"100.64.0.8", "ttl":60
            }),
            ("GET", _) if path.starts_with("/api/dns/zones/") => serde_json::json!({
                "id":"zone/one", "name":"office", "domain":"example.test", "enabled":true,
                "enable_search_domain":false, "distribution_groups":["group-1"], "records":[]
            }),
            ("POST" | "PUT", _) if path.starts_with("/api/dns/zones") => serde_json::json!({
                "id":"zone/one", "name":"office", "domain":"example.test", "enabled":true,
                "enable_search_domain":false, "distribution_groups":["group-1"], "records":[]
            }),
            ("PUT", _) if path.starts_with("/api/accounts/") => serde_json::json!({
                "id":"account/one", "domain":"example.test", "domain_category":"private"
            }),
            _ => panic!("unexpected contract request: {method} {path}"),
        };
        axum::response::Response::builder()
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(response.to_string()))
            .unwrap()
    }

    async fn contract_client() -> (
        NetBirdClient,
        std::sync::Arc<std::sync::Mutex<Vec<CapturedRequest>>>,
        tokio::task::JoinHandle<()>,
    ) {
        use axum::{routing::any, Router};

        let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn({
            let captured = captured.clone();
            async move {
                axum::serve(
                    listener,
                    Router::new()
                        .fallback(any(contract_handler))
                        .with_state(captured),
                )
                .await
                .unwrap();
            }
        });
        let client = NetBirdClient::new(
            NetBirdConfig::new(Auth::oauth_token("contract-token"))
                .with_host(format!("http://{address}")),
        );
        (client, captured, server)
    }

    fn account_update_request() -> AccountRequest {
        AccountRequest {
            settings: AccountSettingsRequest {
                peer_login_expiration_enabled: true,
                peer_login_expiration: 86_400,
                peer_inactivity_expiration_enabled: false,
                peer_inactivity_expiration: 0,
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
        }
    }

    #[tokio::test]
    async fn client_contract_covers_account_peer_posture_and_location_endpoints() {
        let (client, captured, server) = contract_client().await;

        let accessible = client.accessible_peers("peer/one two").await.unwrap();
        assert_eq!(accessible[0].ip.as_deref(), Some("100.64.0.8"));
        assert!(client.posture_checks().await.unwrap().is_empty());
        assert_eq!(
            client
                .posture_check("posture/one")
                .await
                .unwrap()
                .name
                .as_deref(),
            Some("minimum-version")
        );
        let posture_request = PostureCheckRequest {
            name: "minimum-version".into(),
            description: "client version".into(),
            checks: Some(Checks::default()),
        };
        assert_eq!(
            client
                .create_posture_check(&posture_request)
                .await
                .unwrap()
                .id
                .as_deref(),
            Some("posture/one")
        );
        client
            .update_posture_check("posture/one", &posture_request)
            .await
            .unwrap();
        client.delete_posture_check("posture/one").await.unwrap();
        assert_eq!(
            client.countries().await.unwrap()[0].country_code.as_deref(),
            Some("DE")
        );
        assert_eq!(
            client.cities("DE/test").await.unwrap()[0]
                .city_name
                .as_deref(),
            Some("Berlin")
        );
        assert!(client.users(Some(true)).await.unwrap().is_empty());
        client
            .update_account("account/one", &account_update_request())
            .await
            .unwrap();
        client.delete_account("account/one").await.unwrap();

        let captured = captured.lock().unwrap();
        assert!(captured
            .iter()
            .all(|request| { request.authorization.as_deref() == Some("Bearer contract-token") }));
        let account = captured
            .iter()
            .find(|request| request.path == "/api/accounts/account%2Fone")
            .unwrap();
        assert_eq!(account.method, "PUT");
        assert_eq!(account.content_type.as_deref(), Some("application/json"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&account.body).unwrap()["settings"]
                ["peer_login_expiration"],
            86_400
        );
        assert!(captured.iter().any(|request| {
            request.method == "DELETE" && request.path == "/api/accounts/account%2Fone"
        }));
        assert!(captured.iter().any(|request| {
            request.method == "POST"
                && request.path == "/api/posture-checks"
                && serde_json::from_str::<serde_json::Value>(&request.body).unwrap()["checks"]
                    == serde_json::json!({})
        }));
        assert!(captured.iter().any(|request| {
            request.method == "PUT" && request.path == "/api/posture-checks/posture%2Fone"
        }));
        assert!(captured.iter().any(|request| {
            request.method == "DELETE" && request.path == "/api/posture-checks/posture%2Fone"
        }));
        assert!(captured.iter().any(|request| {
            request.method == "GET" && request.path == "/api/locations/countries/DE%2Ftest/cities"
        }));
        assert!(captured.iter().any(|request| {
            request.method == "GET"
                && request.path == "/api/users"
                && request.query.as_deref() == Some("service_user=true")
        }));
        drop(captured);
        server.abort();
    }

    #[tokio::test]
    async fn dns_zone_and_record_crud_escape_every_path_segment() {
        let (client, captured, server) = contract_client().await;
        let zone_request = DnsZoneRequest {
            name: "office".into(),
            domain: "example.test".into(),
            enabled: Some(true),
            enable_search_domain: false,
            distribution_groups: vec!["group-1".into()],
        };
        let record_request = DnsRecordRequest {
            name: "www.example.test".into(),
            record_type: DnsRecordType::A,
            content: "100.64.0.8".into(),
            ttl: 60,
        };

        assert!(client.dns_zones().await.unwrap().is_empty());
        assert_eq!(
            client.dns_zone("zone/one").await.unwrap().domain.as_deref(),
            Some("example.test")
        );
        client.create_dns_zone(&zone_request).await.unwrap();
        client
            .update_dns_zone("zone/one", &zone_request)
            .await
            .unwrap();
        client.delete_dns_zone("zone/one").await.unwrap();
        assert!(client.dns_records("zone/one").await.unwrap().is_empty());
        assert_eq!(
            client
                .dns_record("zone/one", "record/two")
                .await
                .unwrap()
                .ttl,
            Some(60)
        );
        client
            .create_dns_record("zone/one", &record_request)
            .await
            .unwrap();
        client
            .update_dns_record("zone/one", "record/two", &record_request)
            .await
            .unwrap();
        client
            .delete_dns_record("zone/one", "record/two")
            .await
            .unwrap();

        let captured = captured.lock().unwrap();
        let zone_create = captured
            .iter()
            .find(|request| request.method == "POST" && request.path == "/api/dns/zones")
            .unwrap();
        assert_eq!(
            zone_create.content_type.as_deref(),
            Some("application/json")
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&zone_create.body).unwrap(),
            serde_json::json!({
                "name":"office", "domain":"example.test", "enabled":true,
                "enable_search_domain":false, "distribution_groups":["group-1"]
            })
        );
        assert!(captured.iter().any(|request| {
            request.method == "PUT" && request.path == "/api/dns/zones/zone%2Fone"
        }));
        assert!(captured.iter().any(|request| {
            request.method == "DELETE" && request.path == "/api/dns/zones/zone%2Fone"
        }));
        let record_create = captured
            .iter()
            .find(|request| {
                request.method == "POST" && request.path == "/api/dns/zones/zone%2Fone/records"
            })
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&record_create.body).unwrap()["type"],
            "A"
        );
        assert!(captured.iter().any(|request| {
            request.method == "PUT"
                && request.path == "/api/dns/zones/zone%2Fone/records/record%2Ftwo"
        }));
        assert!(captured.iter().any(|request| {
            request.method == "DELETE"
                && request.path == "/api/dns/zones/zone%2Fone/records/record%2Ftwo"
        }));
        drop(captured);
        server.abort();
    }
}
