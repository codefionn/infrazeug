//! Resource adapters for the stable, account-level NetBird API objects.

use crate::NetBirdClientSource;
use async_trait::async_trait;
use infrazeug_ext_netbird_api as api;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

fn missing_id(kind: &str) -> ResourceError {
    ResourceError::provider(format!("NetBird returned a {kind} without an id"))
}
fn drift(parts: Vec<String>) -> Drift {
    if parts.is_empty() {
        Drift::InSync
    } else {
        Drift::Drifted(parts.join(", "))
    }
}
fn string_set(values: &[String]) -> BTreeSet<&str> {
    values.iter().map(String::as_str).collect()
}

/// `None` means the caller does not manage this optional field.
fn optional_string_set_matches(
    current: Option<&Vec<String>>,
    desired: Option<&Vec<String>>,
) -> bool {
    desired.is_none_or(|desired| {
        string_set(current.map(Vec::as_slice).unwrap_or_default()) == string_set(desired)
    })
}

fn string_set_matches(current: &[String], desired: &[String]) -> bool {
    string_set(current) == string_set(desired)
}

async fn resolve_network_id(
    client: &api::NetBirdClient,
    network_id: &str,
    network_name: Option<&str>,
) -> ResourceResult<String> {
    if !network_id.trim().is_empty() {
        return Ok(network_id.to_owned());
    }
    let name = network_name
        .ok_or_else(|| ResourceError::provider("network_id or network_name is required"))?;
    let matches = client
        .networks()
        .await
        .map_err(ResourceError::provider)?
        .into_iter()
        .filter(|network| network.name.as_deref() == Some(name))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [network] => network.id.clone().ok_or_else(|| missing_id("network")),
        [] => Err(ResourceError::provider(format!(
            "no NetBird network named {name:?}"
        ))),
        _ => Err(ResourceError::provider(format!(
            "more than one NetBird network is named {name:?}; use network_id"
        ))),
    }
}

/// Resolve exact group names to their Management API IDs at apply time. This keeps
/// playbooks portable between NetBird accounts, whose group IDs are server generated.
async fn resolve_group_names(
    client: &api::NetBirdClient,
    names: &[String],
) -> ResourceResult<Vec<String>> {
    let mut ids = Vec::with_capacity(names.len());
    for name in names {
        let matches = client
            .groups(Some(name))
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|group| group.minimum.name.as_deref() == Some(name))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [group] => ids.push(
                group
                    .minimum
                    .id
                    .clone()
                    .ok_or_else(|| missing_id("group"))?,
            ),
            [] => {
                return Err(ResourceError::provider(format!(
                    "no NetBird group named {name:?}"
                )))
            }
            _ => {
                return Err(ResourceError::provider(format!(
                    "more than one NetBird group is named {name:?}; use peer_groups"
                )))
            }
        }
    }
    Ok(ids)
}

async fn resolve_peer_names(
    client: &api::NetBirdClient,
    names: &[String],
) -> ResourceResult<Vec<String>> {
    let mut ids = Vec::with_capacity(names.len());
    for name in names {
        let matches = client
            .peers(Some(name), None)
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|peer| peer.name.as_deref() == Some(name))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [peer] => ids.push(peer.id.clone().ok_or_else(|| missing_id("peer"))?),
            [] => {
                return Err(ResourceError::provider(format!(
                    "no NetBird peer named {name:?}"
                )))
            }
            _ => {
                return Err(ResourceError::provider(format!(
                    "more than one NetBird peer is named {name:?}"
                )))
            }
        }
    }
    Ok(ids)
}

// Groups
pub const ENSURE_GROUP: &str = "netbird.ensure_group";
pub type EnsureGroup = EnsureResource<GroupResource>;
pub fn ensure_group(source: NetBirdClientSource) -> EnsureGroup {
    EnsureResource::new(GroupResource { source })
}

/// Desired group. `name` is its stable account-local key.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureGroupInput {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peers: Option<Vec<String>>,
    /// Exact peer names resolved to API IDs when the group is applied. Do not
    /// combine with `peers`, which is retained for callers that know IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_names: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Vec<ResourceRef>>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResourceRef {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureGroupOutput {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peers: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_names: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Vec<ResourceRef>>,
}
impl EnsureGroupInput {
    pub fn to_request(&self) -> api::GroupRequest {
        api::GroupRequest {
            name: self.name.clone(),
            peers: self.peers.clone(),
            resources: self.resources.clone().map(|items| {
                items
                    .into_iter()
                    .map(|item| api::Resource {
                        id: item.id,
                        resource_type: item.resource_type,
                    })
                    .collect()
            }),
        }
    }
}
fn group_state(group: api::Group) -> Option<EnsureGroupOutput> {
    Some(EnsureGroupOutput {
        id: group.minimum.id?,
        name: group.minimum.name?,
        peers: group
            .peers
            .clone()
            .map(|xs| xs.into_iter().filter_map(|x| x.id).collect()),
        peer_names: group
            .peers
            .map(|xs| xs.into_iter().filter_map(|x| x.name).collect()),
        resources: group.resources.map(|xs| {
            xs.into_iter()
                .map(|x| ResourceRef {
                    id: x.id,
                    resource_type: x.resource_type,
                })
                .collect()
        }),
    })
}
async fn resolved_group_input(
    client: &api::NetBirdClient,
    input: &EnsureGroupInput,
) -> ResourceResult<EnsureGroupInput> {
    if input.peers.is_some() && input.peer_names.is_some() {
        return Err(ResourceError::provider(
            "group must use peers or peer_names, not both",
        ));
    }
    let mut resolved = input.clone();
    if let Some(names) = &input.peer_names {
        resolved.peers = Some(resolve_peer_names(client, names).await?);
    }
    resolved.peer_names = None;
    Ok(resolved)
}
#[derive(Clone)]
pub struct GroupResource {
    source: NetBirdClientSource,
}
#[async_trait]
impl Resource for GroupResource {
    type Spec = EnsureGroupInput;
    type State = EnsureGroupOutput;
    fn kind(&self) -> &'static str {
        ENSURE_GROUP
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let matches = self
            .source
            .client(ctx)
            .await?
            .groups(Some(&spec.name))
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|group| group.minimum.name.as_deref() == Some(&spec.name))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [group] => Ok(group_state(group.clone())),
            _ => Err(ResourceError::provider(format!(
                "more than one NetBird group is named {:?}",
                spec.name
            ))),
        }
    }
    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let resolved = resolved_group_input(&client, spec).await?;
        group_state(
            client
                .create_group(&resolved.to_request())
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("group"))
    }
    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let peer_match = if let Some(names) = &spec.peer_names {
            optional_string_set_matches(current.peer_names.as_ref(), Some(names))
        } else {
            optional_string_set_matches(current.peers.as_ref(), spec.peers.as_ref())
        };
        let resources_match = spec.resources.as_ref().is_none_or(|desired| {
            current
                .resources
                .as_ref()
                .map(|xs| {
                    xs.iter()
                        .map(|x| format!("{}:{}", x.resource_type, x.id))
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default()
                == desired
                    .iter()
                    .map(|x| format!("{}:{}", x.resource_type, x.id))
                    .collect::<BTreeSet<_>>()
        });
        drift(
            [
                (!peer_match).then_some("peers".into()),
                (!resources_match).then_some("resources".into()),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let resolved = resolved_group_input(&client, spec).await?;
        group_state(
            client
                .update_group(&current.id, &resolved.to_request())
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("group"))
    }
    async fn delete(&self, ctx: &ResourceCtx, state: &Self::State) -> ResourceResult<()> {
        self.source
            .client(ctx)
            .await?
            .delete_group(&state.id)
            .await
            .map_err(ResourceError::provider)
    }
}

// Identity providers
pub const ENSURE_IDENTITY_PROVIDER: &str = "netbird.ensure_identity_provider";
pub type EnsureIdentityProvider = EnsureResource<IdentityProviderResource>;
pub fn ensure_identity_provider(source: NetBirdClientSource) -> EnsureIdentityProvider {
    EnsureResource::new(IdentityProviderResource { source })
}

/// Location of the write-only identity-provider client secret.
///
/// Use [`IdentityProviderClientSecretSource::MutableVault`] for a client
/// secret generated by another infrazeug node, such as Keycloak. Static vault
/// support remains for secrets managed outside this graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum IdentityProviderClientSecretSource {
    Vault { file: String, field: String },
    MutableVault { file: String, field: String },
}

/// Desired account identity provider. `name` is the stable account-local key.
///
/// The client secret is read only when NetBird needs a create or a non-secret
/// update. The Management API never returns that value, so changing it alone
/// deliberately does not cause an update on every apply.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureIdentityProviderInput {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub issuer: String,
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_source: Option<IdentityProviderClientSecretSource>,
}

/// Observable identity-provider state. NetBird intentionally redacts the
/// client secret, so it is not part of either state or drift detection.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureIdentityProviderOutput {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub issuer: String,
    pub client_id: String,
}

async fn identity_provider_request(
    ctx: &ResourceCtx,
    input: &EnsureIdentityProviderInput,
) -> ResourceResult<api::IdentityProviderRequest> {
    let source = input.client_secret_source.as_ref().ok_or_else(|| {
        ResourceError::provider("identity-provider client_secret_source is required")
    })?;
    let client_secret = match source {
        IdentityProviderClientSecretSource::Vault { file, field } => {
            if file.trim().is_empty() || field.trim().is_empty() {
                return Err(ResourceError::provider(
                    "identity-provider vault client secret file and field must not be empty",
                ));
            }
            ctx.read_secret_string(file, field).await?
        }
        IdentityProviderClientSecretSource::MutableVault { file, field } => {
            if file.trim().is_empty() || field.trim().is_empty() {
                return Err(ResourceError::provider(
                    "identity-provider mutable-vault client secret file and field must not be empty",
                ));
            }
            ctx.read_mutable_secret_string(file, field).await?
        }
    };
    if client_secret.trim().is_empty() {
        return Err(ResourceError::provider(
            "identity-provider client secret is empty",
        ));
    }
    Ok(api::IdentityProviderRequest {
        provider_type: input.provider_type.clone(),
        name: input.name.clone(),
        issuer: input.issuer.clone(),
        client_id: input.client_id.clone(),
        client_secret,
    })
}

fn identity_provider_state(
    provider: api::IdentityProvider,
) -> Option<EnsureIdentityProviderOutput> {
    Some(EnsureIdentityProviderOutput {
        id: provider.id?,
        name: provider.name?,
        provider_type: provider.provider_type?,
        issuer: provider.issuer?,
        client_id: provider.client_id?,
    })
}

#[derive(Clone)]
pub struct IdentityProviderResource {
    source: NetBirdClientSource,
}

#[async_trait]
impl Resource for IdentityProviderResource {
    type Spec = EnsureIdentityProviderInput;
    type State = EnsureIdentityProviderOutput;

    fn kind(&self) -> &'static str {
        ENSURE_IDENTITY_PROVIDER
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let matches = self
            .source
            .client(ctx)
            .await?
            .identity_providers()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|provider| provider.name.as_deref() == Some(&spec.name))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [provider] => identity_provider_state(provider.clone())
                .map(Some)
                .ok_or_else(|| missing_id("identity provider")),
            _ => Err(ResourceError::provider(format!(
                "more than one NetBird identity provider is named {:?}",
                spec.name
            ))),
        }
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let request = identity_provider_request(ctx, spec).await?;
        identity_provider_state(
            self.source
                .client(ctx)
                .await?
                .create_identity_provider(&request)
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("identity provider"))
    }

    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        drift(
            [
                (current.provider_type != spec.provider_type).then_some("type".into()),
                (current.issuer != spec.issuer).then_some("issuer".into()),
                (current.client_id != spec.client_id).then_some("client_id".into()),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
    }

    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let request = identity_provider_request(ctx, spec).await?;
        identity_provider_state(
            self.source
                .client(ctx)
                .await?
                .update_identity_provider(&current.id, &request)
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("identity provider"))
    }

    async fn delete(&self, ctx: &ResourceCtx, state: &Self::State) -> ResourceResult<()> {
        self.source
            .client(ctx)
            .await?
            .delete_identity_provider(&state.id)
            .await
            .map_err(ResourceError::provider)
    }
}

// Networks
pub const ENSURE_NETWORK: &str = "netbird.ensure_network";
pub type EnsureNetwork = EnsureResource<NetworkResource>;
pub fn ensure_network(source: NetBirdClientSource) -> EnsureNetwork {
    EnsureResource::new(NetworkResource { source })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureNetworkInput {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureNetworkOutput {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
impl EnsureNetworkInput {
    pub fn to_request(&self) -> api::NetworkRequest {
        api::NetworkRequest {
            name: self.name.clone(),
            description: self.description.clone(),
        }
    }
}
fn network_state(network: api::Network) -> Option<EnsureNetworkOutput> {
    Some(EnsureNetworkOutput {
        id: network.id?,
        name: network.name?,
        description: network.description,
    })
}
#[derive(Clone)]
pub struct NetworkResource {
    source: NetBirdClientSource,
}
#[async_trait]
impl Resource for NetworkResource {
    type Spec = EnsureNetworkInput;
    type State = EnsureNetworkOutput;
    fn kind(&self) -> &'static str {
        ENSURE_NETWORK
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let matches = self
            .source
            .client(ctx)
            .await?
            .networks()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|n| n.name.as_deref() == Some(&spec.name))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [network] => Ok(network_state(network.clone())),
            _ => Err(ResourceError::provider(format!(
                "more than one NetBird network is named {:?}",
                spec.name
            ))),
        }
    }
    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        network_state(
            self.source
                .client(ctx)
                .await?
                .create_network(&spec.to_request())
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("network"))
    }
    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        drift(
            (spec.description.is_some() && current.description != spec.description)
                .then_some("description".into())
                .into_iter()
                .collect(),
        )
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        network_state(
            self.source
                .client(ctx)
                .await?
                .update_network(&current.id, &spec.to_request())
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("network"))
    }
    async fn delete(&self, ctx: &ResourceCtx, state: &Self::State) -> ResourceResult<()> {
        self.source
            .client(ctx)
            .await?
            .delete_network(&state.id)
            .await
            .map_err(ResourceError::provider)
    }
}

// Routes
pub const ENSURE_ROUTE: &str = "netbird.ensure_route";
pub type EnsureRoute = EnsureResource<RouteResource>;
pub fn ensure_route(source: NetBirdClientSource) -> EnsureRoute {
    EnsureResource::new(RouteResource { source })
}
/// Route identity is network + description. Descriptions must be unique within the network.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureRouteInput {
    pub network_id: String,
    pub description: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_groups: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domains: Option<Vec<String>>,
    #[serde(default = "default_metric")]
    pub metric: u32,
    #[serde(default)]
    pub masquerade: bool,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub keep_route: bool,
    #[serde(default)]
    pub access_control_groups: Vec<String>,
    #[serde(default)]
    pub skip_auto_apply: bool,
}
fn yes() -> bool {
    true
}
fn default_metric() -> u32 {
    9999
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureRouteOutput {
    pub id: String,
    pub network_id: String,
    pub description: String,
    pub enabled: bool,
    pub peer: Option<String>,
    pub peer_groups: Option<Vec<String>>,
    pub network: Option<String>,
    pub domains: Option<Vec<String>>,
    pub metric: u32,
    pub masquerade: bool,
    pub groups: Vec<String>,
    pub keep_route: bool,
    pub access_control_groups: Vec<String>,
    pub skip_auto_apply: bool,
}
impl EnsureRouteInput {
    pub fn to_request(&self, network_id: String) -> api::RouteRequest {
        api::RouteRequest {
            description: self.description.clone(),
            network_id,
            enabled: self.enabled,
            peer: self.peer.clone(),
            peer_groups: self.peer_groups.clone(),
            network: self.network.clone(),
            domains: self.domains.clone(),
            metric: self.metric,
            masquerade: self.masquerade,
            groups: self.groups.clone(),
            keep_route: self.keep_route,
            access_control_groups: Some(self.access_control_groups.clone()),
            skip_auto_apply: Some(self.skip_auto_apply),
        }
    }
}
fn route_state(route: api::Route) -> Option<EnsureRouteOutput> {
    Some(EnsureRouteOutput {
        id: route.id?,
        network_id: route.network_id?,
        description: route.description?,
        enabled: route.enabled.unwrap_or(false),
        peer: route.peer,
        peer_groups: route.peer_groups,
        network: route.network,
        domains: route.domains,
        metric: route.metric.unwrap_or_default(),
        masquerade: route.masquerade.unwrap_or(false),
        groups: route.groups.unwrap_or_default(),
        keep_route: route.keep_route.unwrap_or(false),
        access_control_groups: route.access_control_groups.unwrap_or_default(),
        skip_auto_apply: route.skip_auto_apply.unwrap_or(false),
    })
}
#[derive(Clone)]
pub struct RouteResource {
    source: NetBirdClientSource,
}
#[async_trait]
impl Resource for RouteResource {
    type Spec = EnsureRouteInput;
    type State = EnsureRouteOutput;
    fn kind(&self) -> &'static str {
        ENSURE_ROUTE
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        validate_route(s)?;
        let client = self.source.client(ctx).await?;
        let found = client
            .routes()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|r| {
                r.network_id.as_deref() == Some(&s.network_id)
                    && r.description.as_deref() == Some(&s.description)
            })
            .collect::<Vec<_>>();
        match found.as_slice(){[]=>Ok(None),[route]=>Ok(route_state(route.clone())),_=>Err(ResourceError::provider("more than one NetBird route has this network and description; use a unique description"))}
    }
    async fn create(&self, ctx: &ResourceCtx, s: &Self::Spec) -> ResourceResult<Self::State> {
        validate_route(s)?;
        let client = self.source.client(ctx).await?;
        route_state(
            client
                .create_route(&s.to_request(s.network_id.clone()))
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("route"))
    }
    fn diff(&self, s: &Self::Spec, c: &Self::State) -> Drift {
        let d = [
            (c.enabled != s.enabled, "enabled"),
            (c.peer != s.peer, "peer"),
            (
                !optional_string_set_matches(c.peer_groups.as_ref(), s.peer_groups.as_ref()),
                "peer_groups",
            ),
            (c.network != s.network, "network"),
            (
                !optional_string_set_matches(c.domains.as_ref(), s.domains.as_ref()),
                "domains",
            ),
            (c.metric != s.metric, "metric"),
            (c.masquerade != s.masquerade, "masquerade"),
            (!string_set_matches(&c.groups, &s.groups), "groups"),
            (c.keep_route != s.keep_route, "keep_route"),
            (
                !string_set_matches(&c.access_control_groups, &s.access_control_groups),
                "access_control_groups",
            ),
            (c.skip_auto_apply != s.skip_auto_apply, "skip_auto_apply"),
        ];
        drift(
            d.into_iter()
                .filter_map(|(bad, n)| bad.then_some(n.into()))
                .collect(),
        )
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
        c: Self::State,
    ) -> ResourceResult<Self::State> {
        validate_route(s)?;
        route_state(
            self.source
                .client(ctx)
                .await?
                .update_route(&c.id, &s.to_request(c.network_id.clone()))
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("route"))
    }
    async fn delete(&self, ctx: &ResourceCtx, state: &Self::State) -> ResourceResult<()> {
        self.source
            .client(ctx)
            .await?
            .delete_route(&state.id)
            .await
            .map_err(ResourceError::provider)
    }
}

fn validate_route(spec: &EnsureRouteInput) -> ResourceResult<()> {
    if spec.network_id.trim().is_empty() {
        return Err(ResourceError::provider(
            "route network_id must not be empty",
        ));
    }
    if spec.metric == 0 || spec.metric > 9999 {
        return Err(ResourceError::provider(
            "route metric must be between 1 and 9999",
        ));
    }
    if spec.peer.is_some() == spec.peer_groups.is_some() {
        return Err(ResourceError::provider(
            "route must set exactly one of peer or peer_groups",
        ));
    }
    if spec.network.is_some() == spec.domains.is_some() {
        return Err(ResourceError::provider(
            "route must set exactly one of network or domains",
        ));
    }
    Ok(())
}

// Network resources
pub const ENSURE_NETWORK_RESOURCE: &str = "netbird.ensure_network_resource";
pub type EnsureNetworkResource = EnsureResource<NetworkMemberResource>;
pub fn ensure_network_resource(source: NetBirdClientSource) -> EnsureNetworkResource {
    EnsureResource::new(NetworkMemberResource { source })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnsureNetworkResourceInput {
    #[serde(default)]
    pub network_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_name: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub address: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub groups: Vec<String>,
}
impl Default for EnsureNetworkResourceInput {
    fn default() -> Self {
        Self {
            network_id: String::new(),
            network_name: None,
            name: String::new(),
            description: None,
            address: String::new(),
            enabled: true,
            groups: Vec::new(),
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureNetworkResourceOutput {
    pub id: String,
    pub network_id: String,
    pub name: String,
    pub description: Option<String>,
    pub address: String,
    pub enabled: bool,
    pub groups: Vec<String>,
}
impl EnsureNetworkResourceInput {
    pub fn to_request(&self) -> api::NetworkResourceRequest {
        api::NetworkResourceRequest {
            name: self.name.clone(),
            description: self.description.clone(),
            address: self.address.clone(),
            enabled: self.enabled,
            groups: self.groups.clone(),
        }
    }
}
fn network_resource_state(
    network_id: &str,
    r: api::NetworkResource,
) -> Option<EnsureNetworkResourceOutput> {
    Some(EnsureNetworkResourceOutput {
        id: r.id?,
        network_id: network_id.into(),
        name: r.name?,
        description: r.description,
        address: r.address?,
        enabled: r.enabled.unwrap_or(false),
        groups: r
            .groups
            .unwrap_or_default()
            .into_iter()
            .filter_map(|g| g.id)
            .collect(),
    })
}
#[derive(Clone)]
pub struct NetworkMemberResource {
    source: NetBirdClientSource,
}
#[async_trait]
impl Resource for NetworkMemberResource {
    type Spec = EnsureNetworkResourceInput;
    type State = EnsureNetworkResourceOutput;
    fn kind(&self) -> &'static str {
        ENSURE_NETWORK_RESOURCE
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let network_id =
            resolve_network_id(&client, &s.network_id, s.network_name.as_deref()).await?;
        let matches = client
            .network_resources(&network_id)
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|r| r.name.as_deref() == Some(&s.name))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [resource] => Ok(network_resource_state(&network_id, resource.clone())),
            _ => Err(ResourceError::provider(
                "more than one NetBird network resource has this name; use unique names",
            )),
        }
    }
    async fn create(&self, ctx: &ResourceCtx, s: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let network_id =
            resolve_network_id(&client, &s.network_id, s.network_name.as_deref()).await?;
        network_resource_state(
            &network_id,
            client
                .create_network_resource(&network_id, &s.to_request())
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("network resource"))
    }
    fn diff(&self, s: &Self::Spec, c: &Self::State) -> Drift {
        drift(
            [
                (
                    s.description.is_some() && c.description != s.description,
                    "description",
                ),
                (c.address != s.address, "address"),
                (c.enabled != s.enabled, "enabled"),
                (!string_set_matches(&c.groups, &s.groups), "groups"),
            ]
            .into_iter()
            .filter_map(|(bad, n)| bad.then_some(n.into()))
            .collect(),
        )
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
        c: Self::State,
    ) -> ResourceResult<Self::State> {
        network_resource_state(
            &c.network_id,
            self.source
                .client(ctx)
                .await?
                .update_network_resource(&c.network_id, &c.id, &s.to_request())
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("network resource"))
    }
    async fn delete(&self, ctx: &ResourceCtx, state: &Self::State) -> ResourceResult<()> {
        self.source
            .client(ctx)
            .await?
            .delete_network_resource(&state.network_id, &state.id)
            .await
            .map_err(ResourceError::provider)
    }
}

// Network routers. A router is keyed by its network and its selected peer or peer groups.
pub const ENSURE_NETWORK_ROUTER: &str = "netbird.ensure_network_router";
pub type EnsureNetworkRouter = EnsureResource<NetworkRouterResource>;
pub fn ensure_network_router(source: NetBirdClientSource) -> EnsureNetworkRouter {
    EnsureResource::new(NetworkRouterResource { source })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureNetworkRouterInput {
    #[serde(default)]
    pub network_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    /// Exact peer name resolved to its Management API ID at apply time. Do not
    /// combine with `peer`, `peer_groups`, or `peer_group_names`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_groups: Option<Vec<String>>,
    /// Exact group names resolved to API IDs at apply time. Do not combine with
    /// `peer_groups`, which is retained for callers that already know the IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_group_names: Option<Vec<String>>,
    #[serde(default = "default_metric")]
    pub metric: u32,
    #[serde(default)]
    pub masquerade: bool,
    #[serde(default = "yes")]
    pub enabled: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureNetworkRouterOutput {
    pub id: String,
    pub network_id: String,
    pub peer: Option<String>,
    pub peer_groups: Option<Vec<String>>,
    pub metric: u32,
    pub masquerade: bool,
    pub enabled: bool,
    /// Exact-selector duplicates observed on the Management API. These are
    /// removed by `reconcile`, retaining the router with the lowest API ID.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub duplicate_ids: Vec<String>,
}
impl EnsureNetworkRouterInput {
    pub fn to_request(&self) -> api::NetworkRouterRequest {
        api::NetworkRouterRequest {
            peer: self.peer.clone(),
            peer_groups: self.peer_groups.clone(),
            metric: self.metric,
            masquerade: self.masquerade,
            enabled: self.enabled,
        }
    }
}
async fn resolved_router_input(
    client: &api::NetBirdClient,
    input: &EnsureNetworkRouterInput,
) -> ResourceResult<EnsureNetworkRouterInput> {
    let selectors = usize::from(input.peer.is_some())
        + usize::from(input.peer_name.is_some())
        + usize::from(input.peer_groups.is_some())
        + usize::from(input.peer_group_names.is_some());
    if selectors != 1 {
        return Err(ResourceError::provider(
            "network router must set exactly one of peer, peer_name, peer_groups, or peer_group_names",
        ));
    }
    let mut resolved = input.clone();
    if let Some(name) = &input.peer_name {
        resolved.peer =
            Some(resolve_peer_names(client, std::slice::from_ref(name)).await?[0].clone());
    }
    if let Some(names) = &input.peer_group_names {
        resolved.peer_groups = Some(resolve_group_names(client, names).await?);
    }
    resolved.peer_name = None;
    resolved.peer_group_names = None;
    Ok(resolved)
}
fn router_state(
    network_id: &str,
    r: api::NetworkRouter,
    duplicate_ids: Vec<String>,
) -> Option<EnsureNetworkRouterOutput> {
    Some(EnsureNetworkRouterOutput {
        id: r.id?,
        network_id: network_id.into(),
        peer: r.peer,
        peer_groups: r.peer_groups,
        metric: r.metric.unwrap_or_default(),
        masquerade: r.masquerade.unwrap_or(false),
        enabled: r.enabled.unwrap_or(false),
        duplicate_ids,
    })
}
#[derive(Clone)]
pub struct NetworkRouterResource {
    source: NetBirdClientSource,
}
#[async_trait]
impl Resource for NetworkRouterResource {
    type Spec = EnsureNetworkRouterInput;
    type State = EnsureNetworkRouterOutput;
    fn kind(&self) -> &'static str {
        ENSURE_NETWORK_ROUTER
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let s = resolved_router_input(&client, s).await?;
        validate_router(&s)?;
        let network_id =
            resolve_network_id(&client, &s.network_id, s.network_name.as_deref()).await?;
        let mut matches = client
            .network_routers(&network_id)
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|r| {
                r.peer == s.peer
                    && optional_string_set_matches(r.peer_groups.as_ref(), s.peer_groups.as_ref())
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| left.id.cmp(&right.id));
        match matches.as_slice() {
            [] => Ok(None),
            [router] => Ok(router_state(&network_id, router.clone(), Vec::new())),
            [router, duplicates @ ..] => {
                let duplicate_ids = duplicates
                    .iter()
                    .map(|router| {
                        router
                            .id
                            .clone()
                            .ok_or_else(|| missing_id("network router"))
                    })
                    .collect::<ResourceResult<Vec<_>>>()?;
                Ok(router_state(&network_id, router.clone(), duplicate_ids))
            }
        }
    }
    async fn create(&self, ctx: &ResourceCtx, s: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let s = resolved_router_input(&client, s).await?;
        validate_router(&s)?;
        let network_id =
            resolve_network_id(&client, &s.network_id, s.network_name.as_deref()).await?;
        router_state(
            &network_id,
            client
                .create_network_router(&network_id, &s.to_request())
                .await
                .map_err(ResourceError::provider)?,
            Vec::new(),
        )
        .ok_or_else(|| missing_id("network router"))
    }
    fn diff(&self, s: &Self::Spec, c: &Self::State) -> Drift {
        drift(
            [
                (c.metric != s.metric, "metric"),
                (c.masquerade != s.masquerade, "masquerade"),
                (c.enabled != s.enabled, "enabled"),
                (!c.duplicate_ids.is_empty(), "duplicate routers"),
            ]
            .into_iter()
            .filter_map(|(bad, n)| bad.then_some(n.into()))
            .collect(),
        )
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
        c: Self::State,
    ) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let s = resolved_router_input(&client, s).await?;
        validate_router(&s)?;
        let updated = client
            .update_network_router(&c.network_id, &c.id, &s.to_request())
            .await
            .map_err(ResourceError::provider)?;
        for duplicate_id in &c.duplicate_ids {
            client
                .delete_network_router(&c.network_id, duplicate_id)
                .await
                .map_err(ResourceError::provider)?;
        }
        router_state(&c.network_id, updated, Vec::new()).ok_or_else(|| missing_id("network router"))
    }
    async fn delete(&self, ctx: &ResourceCtx, state: &Self::State) -> ResourceResult<()> {
        self.source
            .client(ctx)
            .await?
            .delete_network_router(&state.network_id, &state.id)
            .await
            .map_err(ResourceError::provider)
    }
}

fn validate_router(spec: &EnsureNetworkRouterInput) -> ResourceResult<()> {
    if spec.metric == 0 || spec.metric > 9999 {
        return Err(ResourceError::provider(
            "network router metric must be between 1 and 9999",
        ));
    }
    let selectors = usize::from(spec.peer.is_some())
        + usize::from(spec.peer_name.is_some())
        + usize::from(spec.peer_groups.is_some())
        + usize::from(spec.peer_group_names.is_some());
    if selectors != 1 {
        return Err(ResourceError::provider(
            "network router must set exactly one of peer, peer_name, peer_groups, or peer_group_names",
        ));
    }
    Ok(())
}

// DNS nameserver groups
pub const ENSURE_NAMESERVER_GROUP: &str = "netbird.ensure_nameserver_group";
pub type EnsureNameserverGroup = EnsureResource<NameserverGroupResource>;
pub fn ensure_nameserver_group(source: NetBirdClientSource) -> EnsureNameserverGroup {
    EnsureResource::new(NameserverGroupResource { source })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct NameserverInput {
    pub ip: String,
    pub ns_type: String,
    pub port: u16,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureNameserverGroupInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub nameservers: Vec<NameserverInput>,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub groups: Vec<String>,
    /// Exact NetBird group names resolved to Management API IDs when the
    /// nameserver group is applied. Do not combine with `groups`, which is
    /// retained for callers that already know account-local IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_names: Option<Vec<String>>,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub search_domains_enabled: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureNameserverGroupOutput {
    pub id: String,
    pub name: String,
    pub description: String,
    pub nameservers: Vec<NameserverInput>,
    pub enabled: bool,
    pub groups: Vec<String>,
    /// Group names mapped from the IDs returned by the Management API. This
    /// is present when the desired input selected groups by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_names: Option<Vec<String>>,
    pub primary: bool,
    pub domains: Vec<String>,
    pub search_domains_enabled: bool,
}
impl EnsureNameserverGroupInput {
    pub fn to_request(&self) -> api::NameserverGroupRequest {
        api::NameserverGroupRequest {
            name: self.name.clone(),
            description: self.description.clone(),
            nameservers: self
                .nameservers
                .iter()
                .map(|n| api::Nameserver {
                    ip: n.ip.clone(),
                    ns_type: n.ns_type.clone(),
                    port: n.port,
                })
                .collect(),
            enabled: self.enabled,
            groups: self.groups.clone(),
            primary: self.primary,
            domains: self.domains.clone(),
            search_domains_enabled: self.search_domains_enabled,
        }
    }
}
async fn nameserver_group_state(
    client: &api::NetBirdClient,
    group: api::NameserverGroup,
    include_group_names: bool,
) -> ResourceResult<Option<EnsureNameserverGroupOutput>> {
    let Some(id) = group.id else {
        return Ok(None);
    };
    let Some(name) = group.name else {
        return Ok(None);
    };
    let groups = group.groups.unwrap_or_default();
    let group_names = if include_group_names {
        let mut names = Vec::with_capacity(groups.len());
        for id in &groups {
            let resolved = client.group(id).await.map_err(ResourceError::provider)?;
            names.push(resolved.minimum.name.ok_or_else(|| missing_id("group"))?);
        }
        Some(names)
    } else {
        None
    };
    Ok(Some(EnsureNameserverGroupOutput {
        id,
        name,
        description: group.description.unwrap_or_default(),
        nameservers: group
            .nameservers
            .unwrap_or_default()
            .into_iter()
            .map(|n| NameserverInput {
                ip: n.ip,
                ns_type: n.ns_type,
                port: n.port,
            })
            .collect(),
        enabled: group.enabled.unwrap_or(false),
        groups,
        group_names,
        primary: group.primary.unwrap_or(false),
        domains: group.domains.unwrap_or_default(),
        search_domains_enabled: group.search_domains_enabled.unwrap_or(false),
    }))
}

async fn resolved_nameserver_group_input(
    client: &api::NetBirdClient,
    input: &EnsureNameserverGroupInput,
) -> ResourceResult<EnsureNameserverGroupInput> {
    if !input.groups.is_empty() && input.group_names.is_some() {
        return Err(ResourceError::provider(
            "nameserver group must use groups or group_names, not both",
        ));
    }
    let mut resolved = input.clone();
    if let Some(names) = &input.group_names {
        resolved.groups = resolve_group_names(client, names).await?;
    }
    resolved.group_names = None;
    Ok(resolved)
}
#[derive(Clone)]
pub struct NameserverGroupResource {
    source: NetBirdClientSource,
}
#[async_trait]
impl Resource for NameserverGroupResource {
    type Spec = EnsureNameserverGroupInput;
    type State = EnsureNameserverGroupOutput;
    fn kind(&self) -> &'static str {
        ENSURE_NAMESERVER_GROUP
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        validate_nameserver_group(s)?;
        let client = self.source.client(ctx).await?;
        let matches = client
            .nameserver_groups()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|g| g.name.as_deref() == Some(&s.name))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [group] => {
                nameserver_group_state(&client, group.clone(), s.group_names.is_some()).await
            }
            _ => Err(ResourceError::provider(format!(
                "more than one NetBird nameserver group is named {:?}",
                s.name
            ))),
        }
    }
    async fn create(&self, ctx: &ResourceCtx, s: &Self::Spec) -> ResourceResult<Self::State> {
        validate_nameserver_group(s)?;
        let client = self.source.client(ctx).await?;
        let resolved = resolved_nameserver_group_input(&client, s).await?;
        nameserver_group_state(
            &client,
            client
                .create_nameserver_group(&resolved.to_request())
                .await
                .map_err(ResourceError::provider)?,
            s.group_names.is_some(),
        )
        .await?
        .ok_or_else(|| missing_id("nameserver group"))
    }
    fn diff(&self, s: &Self::Spec, c: &Self::State) -> Drift {
        let groups_match = if let Some(names) = &s.group_names {
            optional_string_set_matches(c.group_names.as_ref(), Some(names))
        } else {
            string_set_matches(&c.groups, &s.groups)
        };
        drift(
            [
                (c.description != s.description, "description"),
                (
                    c.nameservers.iter().collect::<BTreeSet<_>>()
                        != s.nameservers.iter().collect::<BTreeSet<_>>(),
                    "nameservers",
                ),
                (c.enabled != s.enabled, "enabled"),
                (!groups_match, "groups"),
                (c.primary != s.primary, "primary"),
                (!string_set_matches(&c.domains, &s.domains), "domains"),
                (
                    c.search_domains_enabled != s.search_domains_enabled,
                    "search_domains_enabled",
                ),
            ]
            .into_iter()
            .filter_map(|(bad, n)| bad.then_some(n.into()))
            .collect(),
        )
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
        c: Self::State,
    ) -> ResourceResult<Self::State> {
        validate_nameserver_group(s)?;
        let client = self.source.client(ctx).await?;
        let resolved = resolved_nameserver_group_input(&client, s).await?;
        nameserver_group_state(
            &client,
            client
                .update_nameserver_group(&c.id, &resolved.to_request())
                .await
                .map_err(ResourceError::provider)?,
            s.group_names.is_some(),
        )
        .await?
        .ok_or_else(|| missing_id("nameserver group"))
    }
    async fn delete(&self, ctx: &ResourceCtx, state: &Self::State) -> ResourceResult<()> {
        self.source
            .client(ctx)
            .await?
            .delete_nameserver_group(&state.id)
            .await
            .map_err(ResourceError::provider)
    }
}

fn validate_nameserver_group(spec: &EnsureNameserverGroupInput) -> ResourceResult<()> {
    if !spec.groups.is_empty() && spec.group_names.is_some() {
        return Err(ResourceError::provider(
            "nameserver group must use groups or group_names, not both",
        ));
    }
    if !(1..=3).contains(&spec.nameservers.len()) {
        return Err(ResourceError::provider(
            "nameserver group must contain between one and three nameservers",
        ));
    }
    if spec
        .nameservers
        .iter()
        .any(|server| server.ns_type != "udp")
    {
        return Err(ResourceError::provider(
            "NetBird nameservers currently require ns_type \"udp\"",
        ));
    }
    if spec.primary == !spec.domains.is_empty() {
        return Err(ResourceError::provider(
            "primary nameserver groups require no domains; non-primary groups require domains",
        ));
    }
    if spec.search_domains_enabled && spec.domains.is_empty() {
        return Err(ResourceError::provider(
            "search_domains_enabled requires at least one domain",
        ));
    }
    Ok(())
}

// Account-level DNS settings are a singleton. They are never created or deleted.
pub const ENSURE_DNS_SETTINGS: &str = "netbird.ensure_dns_settings";
pub type EnsureDnsSettings = EnsureResource<DnsSettingsResource>;
pub fn ensure_dns_settings(source: NetBirdClientSource) -> EnsureDnsSettings {
    EnsureResource::new(DnsSettingsResource { source })
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureDnsSettingsInput {
    #[serde(default)]
    pub disabled_management_groups: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureDnsSettingsOutput {
    pub disabled_management_groups: Vec<String>,
}
#[derive(Clone)]
pub struct DnsSettingsResource {
    source: NetBirdClientSource,
}
#[async_trait]
impl Resource for DnsSettingsResource {
    type Spec = EnsureDnsSettingsInput;
    type State = EnsureDnsSettingsOutput;
    fn kind(&self) -> &'static str {
        ENSURE_DNS_SETTINGS
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        _: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let s = self
            .source
            .client(ctx)
            .await?
            .dns_settings()
            .await
            .map_err(ResourceError::provider)?;
        Ok(Some(EnsureDnsSettingsOutput {
            disabled_management_groups: s.disabled_management_groups,
        }))
    }
    async fn create(&self, ctx: &ResourceCtx, s: &Self::Spec) -> ResourceResult<Self::State> {
        self.reconcile(ctx, s, EnsureDnsSettingsOutput::default())
            .await
    }
    fn diff(&self, s: &Self::Spec, c: &Self::State) -> Drift {
        drift(
            (!string_set_matches(&c.disabled_management_groups, &s.disabled_management_groups))
                .then_some("disabled_management_groups".into())
                .into_iter()
                .collect(),
        )
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
        _: Self::State,
    ) -> ResourceResult<Self::State> {
        let result = self
            .source
            .client(ctx)
            .await?
            .update_dns_settings(&api::DnsSettings {
                disabled_management_groups: s.disabled_management_groups.clone(),
            })
            .await
            .map_err(ResourceError::provider)?;
        Ok(EnsureDnsSettingsOutput {
            disabled_management_groups: result.disabled_management_groups,
        })
    }
}

// Policies intentionally model only group-to-group rules. Resource selectors and
// authorization maps have server-side shapes that vary across NetBird releases.
pub const ENSURE_POLICY: &str = "netbird.ensure_policy";
pub type EnsurePolicy = EnsureResource<PolicyResource>;
pub fn ensure_policy(source: NetBirdClientSource) -> EnsurePolicy {
    EnsureResource::new(PolicyResource { source })
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRuleInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "accept")]
    pub action: String,
    #[serde(default)]
    pub bidirectional: bool,
    #[serde(default = "all")]
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,
    #[serde(default)]
    pub sources: Vec<String>,
    /// Exact source group names, resolved to IDs only when the policy is applied.
    /// Do not combine with `sources`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_group_names: Option<Vec<String>>,
    /// A single network resource may be the source instead of source groups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_resource: Option<PolicyNetworkResourceRef>,
    #[serde(default)]
    pub destinations: Vec<String>,
    /// Exact destination group names, resolved to IDs only when the policy is applied.
    /// Do not combine with `destinations`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_group_names: Option<Vec<String>>,
    /// A single network resource may be the destination instead of destination groups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_resource: Option<PolicyNetworkResourceRef>,
}

impl Default for PolicyRuleInput {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            description: None,
            enabled: true,
            action: accept(),
            bidirectional: false,
            protocol: all(),
            ports: None,
            sources: Vec::new(),
            source_group_names: None,
            source_resource: None,
            destinations: Vec::new(),
            destination_group_names: None,
            destination_resource: None,
        }
    }
}

/// A Network resource selected by its account-local network and resource names.
/// `resource_id` and `resource_type` retain support for callers that already have
/// Management API IDs, but normal playbooks should use the names.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyNetworkResourceRef {
    #[serde(default)]
    pub network_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_name: Option<String>,
    #[serde(default)]
    pub resource_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
}
fn accept() -> String {
    "accept".into()
}
fn all() -> String {
    "all".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnsurePolicyInput {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub rules: Vec<PolicyRuleInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_posture_checks: Option<Vec<String>>,
}

impl Default for EnsurePolicyInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            enabled: true,
            rules: Vec::new(),
            source_posture_checks: None,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsurePolicyOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub rules: Vec<PolicyRuleInput>,
    pub source_posture_checks: Option<Vec<String>>,
}
impl EnsurePolicyInput {
    pub fn to_request(&self) -> api::PolicyRequest {
        api::PolicyRequest {
            name: self.name.clone(),
            description: self.description.clone(),
            enabled: self.enabled,
            rules: self
                .rules
                .iter()
                .map(|r| {
                    let source_resource =
                        r.source_resource.as_ref().map(|resource| api::Resource {
                            id: resource.resource_id.clone(),
                            resource_type: resource.resource_type.clone().unwrap_or_default(),
                        });
                    let destination_resource =
                        r.destination_resource
                            .as_ref()
                            .map(|resource| api::Resource {
                                id: resource.resource_id.clone(),
                                resource_type: resource.resource_type.clone().unwrap_or_default(),
                            });
                    api::PolicyRuleRequest {
                        id: r.id.clone(),
                        name: r.name.clone(),
                        description: r.description.clone(),
                        enabled: r.enabled,
                        action: r.action.clone(),
                        bidirectional: r.bidirectional,
                        protocol: r.protocol.clone(),
                        ports: r.ports.clone(),
                        port_ranges: None,
                        authorized_groups: None,
                        sources: (source_resource.is_none() && !r.sources.is_empty())
                            .then(|| r.sources.clone()),
                        source_resource,
                        destinations: (destination_resource.is_none()
                            && !r.destinations.is_empty())
                        .then(|| r.destinations.clone()),
                        destination_resource,
                    }
                })
                .collect(),
            source_posture_checks: self.source_posture_checks.clone(),
        }
    }
}

async fn resolve_policy_resource(
    client: &api::NetBirdClient,
    resource: &PolicyNetworkResourceRef,
) -> ResourceResult<PolicyNetworkResourceRef> {
    if !resource.resource_id.trim().is_empty() {
        if resource.resource_type.as_deref().is_none_or(str::is_empty) {
            return Err(ResourceError::provider(
                "policy resource_id requires resource_type",
            ));
        }
        return Ok(resource.clone());
    }
    let network_id = resolve_network_id(
        client,
        &resource.network_id,
        resource.network_name.as_deref(),
    )
    .await?;
    let resource_name = resource.resource_name.as_deref().ok_or_else(|| {
        ResourceError::provider("policy resource_id or resource_name is required")
    })?;
    let matches = client
        .network_resources(&network_id)
        .await
        .map_err(ResourceError::provider)?
        .into_iter()
        .filter(|candidate| candidate.name.as_deref() == Some(resource_name))
        .collect::<Vec<_>>();
    let candidate = match matches.as_slice() {
        [candidate] => candidate,
        [] => {
            return Err(ResourceError::provider(format!(
                "no NetBird network resource named {resource_name:?}"
            )))
        }
        _ => {
            return Err(ResourceError::provider(format!(
                "more than one NetBird network resource is named {resource_name:?}"
            )))
        }
    };
    let mut resolved = resource.clone();
    resolved.network_id = network_id;
    resolved.resource_id = candidate
        .id
        .clone()
        .ok_or_else(|| missing_id("network resource"))?;
    let actual_type = candidate.resource_type.clone().ok_or_else(|| {
        ResourceError::provider("NetBird returned a network resource without a type")
    })?;
    if let Some(expected_type) = &resource.resource_type {
        if expected_type != &actual_type {
            return Err(ResourceError::provider(format!(
                "policy resource {resource_name:?} has type {actual_type:?}, not {expected_type:?}"
            )));
        }
    }
    resolved.resource_type = Some(actual_type);
    Ok(resolved)
}

async fn resolved_policy_input(
    client: &api::NetBirdClient,
    input: &EnsurePolicyInput,
) -> ResourceResult<EnsurePolicyInput> {
    let mut resolved = input.clone();
    for rule in &mut resolved.rules {
        if !rule.sources.is_empty() && rule.source_group_names.is_some() {
            return Err(ResourceError::provider(
                "policy rule must use sources or source_group_names, not both",
            ));
        }
        if !rule.destinations.is_empty() && rule.destination_group_names.is_some() {
            return Err(ResourceError::provider(
                "policy rule must use destinations or destination_group_names, not both",
            ));
        }
        if let Some(names) = &rule.source_group_names {
            rule.sources = resolve_group_names(client, names).await?;
        }
        if let Some(names) = &rule.destination_group_names {
            rule.destinations = resolve_group_names(client, names).await?;
        }
        rule.source_group_names = None;
        rule.destination_group_names = None;
        if let Some(resource) = &rule.source_resource {
            rule.source_resource = Some(resolve_policy_resource(client, resource).await?);
        }
        if let Some(resource) = &rule.destination_resource {
            rule.destination_resource = Some(resolve_policy_resource(client, resource).await?);
        }
    }
    Ok(resolved)
}

fn normalized_policy_rules(rules: &[PolicyRuleInput]) -> Vec<PolicyRuleInput> {
    let mut normalized = rules.to_vec();
    for rule in &mut normalized {
        rule.id = None;
        rule.sources.sort();
        rule.sources.dedup();
        if rule.source_group_names.is_some() {
            rule.sources.clear();
        }
        if let Some(names) = &mut rule.source_group_names {
            names.sort();
            names.dedup();
        }
        rule.destinations.sort();
        rule.destinations.dedup();
        if rule.destination_group_names.is_some() {
            rule.destinations.clear();
        }
        if let Some(names) = &mut rule.destination_group_names {
            names.sort();
            names.dedup();
        }
        if let Some(ports) = &mut rule.ports {
            ports.sort();
            ports.dedup();
        }
    }
    normalized.sort_by(|left, right| left.name.cmp(&right.name));
    normalized
}

fn policy_rules_match(current: &[PolicyRuleInput], desired: &[PolicyRuleInput]) -> bool {
    let mut current = current.to_vec();
    for current_rule in &mut current {
        let Some(desired_rule) = desired.iter().find(|rule| rule.name == current_rule.name) else {
            continue;
        };
        if desired_rule.source_group_names.is_some() {
            current_rule.sources.clear();
        } else {
            current_rule.source_group_names = None;
        }
        if desired_rule.destination_group_names.is_some() {
            current_rule.destinations.clear();
        } else {
            current_rule.destination_group_names = None;
        }
        // A resource selected by name has already been resolved against the
        // account when the request was made. Its API ID cannot be compared in
        // this synchronous diff hook, so compare the remaining rule settings.
        if desired_rule
            .source_resource
            .as_ref()
            .is_some_and(|resource| resource.resource_name.is_some())
        {
            current_rule.source_resource = desired_rule.source_resource.clone();
        }
        if desired_rule
            .destination_resource
            .as_ref()
            .is_some_and(|resource| resource.resource_name.is_some())
        {
            current_rule.destination_resource = desired_rule.destination_resource.clone();
        }
    }
    normalized_policy_rules(&current) == normalized_policy_rules(desired)
}

fn validate_policy(spec: &EnsurePolicyInput) -> ResourceResult<()> {
    let mut names = BTreeSet::new();
    for rule in &spec.rules {
        if rule.name.trim().is_empty() {
            return Err(ResourceError::provider(
                "policy rule names must not be empty",
            ));
        }
        if !names.insert(rule.name.as_str()) {
            return Err(ResourceError::provider(format!(
                "policy {:?} contains duplicate rule name {:?}",
                spec.name, rule.name
            )));
        }
    }
    Ok(())
}

fn validate_supported_policy(policy: &api::Policy) -> ResourceResult<()> {
    let unsupported = policy
        .rules
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|rule| {
            rule.port_ranges
                .as_ref()
                .is_some_and(|ranges| !ranges.is_empty())
                || rule
                    .authorized_groups
                    .as_ref()
                    .is_some_and(|groups| !groups.is_empty())
        });
    if unsupported {
        Err(ResourceError::provider(format!(
            "NetBird policy {:?} uses port ranges or authorized groups, which infrazeug-netbird does not manage",
            policy.name.as_deref().unwrap_or("<unnamed>")
        )))
    } else {
        Ok(())
    }
}

fn policy_state(p: api::Policy) -> Option<EnsurePolicyOutput> {
    Some(EnsurePolicyOutput {
        id: p.id?,
        name: p.name?,
        description: p.description,
        enabled: p.enabled.unwrap_or(false),
        rules: p
            .rules
            .unwrap_or_default()
            .into_iter()
            .map(|r| PolicyRuleInput {
                id: r.id,
                name: r.name.unwrap_or_default(),
                description: r.description,
                enabled: r.enabled.unwrap_or(false),
                action: r.action.unwrap_or_default(),
                bidirectional: r.bidirectional.unwrap_or(false),
                protocol: r.protocol.unwrap_or_default(),
                ports: r.ports,
                sources: r
                    .sources
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|g| g.id)
                    .collect(),
                source_group_names: r
                    .sources
                    .map(|groups| groups.into_iter().filter_map(|g| g.name).collect()),
                source_resource: r.source_resource.map(|resource| PolicyNetworkResourceRef {
                    resource_id: resource.id,
                    resource_type: Some(resource.resource_type),
                    ..Default::default()
                }),
                destinations: r
                    .destinations
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|g| g.id)
                    .collect(),
                destination_group_names: r
                    .destinations
                    .map(|groups| groups.into_iter().filter_map(|g| g.name).collect()),
                destination_resource: r.destination_resource.map(|resource| {
                    PolicyNetworkResourceRef {
                        resource_id: resource.id,
                        resource_type: Some(resource.resource_type),
                        ..Default::default()
                    }
                }),
            })
            .collect(),
        source_posture_checks: p.source_posture_checks,
    })
}
#[derive(Clone)]
pub struct PolicyResource {
    source: NetBirdClientSource,
}
#[async_trait]
impl Resource for PolicyResource {
    type Spec = EnsurePolicyInput;
    type State = EnsurePolicyOutput;
    fn kind(&self) -> &'static str {
        ENSURE_POLICY
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        validate_policy(s)?;
        let matches = self
            .source
            .client(ctx)
            .await?
            .policies()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|policy| policy.name.as_deref() == Some(&s.name))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [policy] => {
                validate_supported_policy(policy)?;
                Ok(policy_state(policy.clone()))
            }
            _ => Err(ResourceError::provider(format!(
                "more than one NetBird policy is named {:?}",
                s.name
            ))),
        }
    }
    async fn create(&self, ctx: &ResourceCtx, s: &Self::Spec) -> ResourceResult<Self::State> {
        validate_policy(s)?;
        let client = self.source.client(ctx).await?;
        let resolved = resolved_policy_input(&client, s).await?;
        policy_state(
            client
                .create_policy(&resolved.to_request())
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("policy"))
    }
    fn diff(&self, s: &Self::Spec, c: &Self::State) -> Drift {
        drift(
            [
                (
                    s.description.is_some() && c.description != s.description,
                    "description",
                ),
                (c.enabled != s.enabled, "enabled"),
                (
                    !optional_string_set_matches(
                        c.source_posture_checks.as_ref(),
                        s.source_posture_checks.as_ref(),
                    ),
                    "source_posture_checks",
                ),
                (!policy_rules_match(&c.rules, &s.rules), "rules"),
            ]
            .into_iter()
            .filter_map(|(bad, n)| bad.then_some(n.into()))
            .collect(),
        )
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
        c: Self::State,
    ) -> ResourceResult<Self::State> {
        validate_policy(s)?;
        let client = self.source.client(ctx).await?;
        let resolved = resolved_policy_input(&client, s).await?;
        let mut request = resolved.to_request();
        for rule in &mut request.rules {
            if rule.id.is_some() {
                continue;
            }
            let matches = c
                .rules
                .iter()
                .filter(|current| current.name == rule.name)
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [] => {}
                [current] => rule.id.clone_from(&current.id),
                _ => {
                    return Err(ResourceError::provider(format!(
                        "NetBird policy {:?} has duplicate live rule name {:?}",
                        s.name, rule.name
                    )))
                }
            }
        }
        policy_state(
            client
                .update_policy(&c.id, &request)
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("policy"))
    }
    async fn delete(&self, ctx: &ResourceCtx, state: &Self::State) -> ResourceResult<()> {
        self.source
            .client(ctx)
            .await?
            .delete_policy(&state.id)
            .await
            .map_err(ResourceError::provider)
    }
}

// One-time credentials
//
// The plaintext fields below are intentionally serialized because a downstream
// mutable-vault write needs the node capture. They never implement `Debug`, and
// every observed state has the plaintext set to `None`.
async fn mutable_vault_field_present(
    ctx: &ResourceCtx,
    file: &str,
    field: &str,
) -> ResourceResult<bool> {
    if file.trim().is_empty() || field.trim().is_empty() {
        return Err(ResourceError::provider(
            "mutable_vault_file and mutable_vault_field must not be empty",
        ));
    }
    if !ctx.has_mutable_secrets() {
        return Err(ResourceError::InputsUnavailable);
    }
    match ctx.read_mutable_secret(file, field).await {
        Ok(value) => Ok(!value.is_empty()),
        Err(ResourceError::Provider(message))
            if message.contains("missing vault file") || message.contains("missing field") =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

pub const ENSURE_SETUP_KEY: &str = "netbird.ensure_setup_key";
pub type EnsureSetupKey = EnsureResource<SetupKeyResource>;
pub fn ensure_setup_key(source: NetBirdClientSource) -> EnsureSetupKey {
    EnsureResource::new(SetupKeyResource { source })
}

/// Desired NetBird setup key and the mutable-vault field that must hold its
/// plaintext. A valid API key without that field is revoked and replaced, which
/// closes the crash gap between creation and capture.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureSetupKeyInput {
    pub name: String,
    #[serde(default = "reusable")]
    pub key_type: String,
    pub expires_in: u64,
    #[serde(default)]
    pub auto_groups: Vec<String>,
    /// Exact group names resolved to Management API IDs on creation. Do not
    /// combine with `auto_groups`, which is retained for callers with IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_group_names: Option<Vec<String>>,
    #[serde(default)]
    pub usage_limit: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ephemeral: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_extra_dns_labels: Option<bool>,
    pub mutable_vault_file: String,
    pub mutable_vault_field: String,
}
fn reusable() -> String {
    "reusable".into()
}
#[derive(Clone, Serialize, Deserialize)]
pub struct EnsureSetupKeyOutput {
    pub id: String,
    pub name: String,
    pub key_type: String,
    pub valid: bool,
    pub revoked: bool,
    pub used_times: u64,
    pub usage_limit: u64,
    pub auto_groups: Vec<String>,
    pub ephemeral: Option<bool>,
    pub allow_extra_dns_labels: Option<bool>,
    /// Present only in the creation capture. JSON pointer: `/key`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default)]
    reissue: bool,
}
impl std::fmt::Debug for EnsureSetupKeyOutput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EnsureSetupKeyOutput")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("key_type", &self.key_type)
            .field("valid", &self.valid)
            .field("revoked", &self.revoked)
            .field("used_times", &self.used_times)
            .field("usage_limit", &self.usage_limit)
            .field("auto_groups", &self.auto_groups)
            .field("ephemeral", &self.ephemeral)
            .field("allow_extra_dns_labels", &self.allow_extra_dns_labels)
            .field("key", &self.key.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}
impl EnsureSetupKeyInput {
    fn to_request(&self) -> api::CreateSetupKeyRequest {
        api::CreateSetupKeyRequest {
            name: self.name.clone(),
            key_type: self.key_type.clone(),
            expires_in: self.expires_in,
            auto_groups: self.auto_groups.clone(),
            usage_limit: self.usage_limit,
            ephemeral: self.ephemeral,
            allow_extra_dns_labels: self.allow_extra_dns_labels,
        }
    }
}
async fn resolved_setup_key_input(
    client: &api::NetBirdClient,
    input: &EnsureSetupKeyInput,
) -> ResourceResult<EnsureSetupKeyInput> {
    if !input.auto_groups.is_empty() && input.auto_group_names.is_some() {
        return Err(ResourceError::provider(
            "setup key must use auto_groups or auto_group_names, not both",
        ));
    }
    let mut resolved = input.clone();
    if let Some(names) = &input.auto_group_names {
        resolved.auto_groups = resolve_group_names(client, names).await?;
    }
    resolved.auto_group_names = None;
    Ok(resolved)
}
fn setup_key_state(
    key: api::SetupKey,
    plaintext: Option<String>,
    reissue: bool,
) -> Option<EnsureSetupKeyOutput> {
    Some(EnsureSetupKeyOutput {
        id: key.id?,
        name: key.name?,
        key_type: key.key_type?,
        valid: key.valid.unwrap_or(false),
        revoked: key.revoked.unwrap_or(false),
        used_times: key.used_times.unwrap_or_default(),
        usage_limit: key.usage_limit.unwrap_or_default(),
        auto_groups: key.auto_groups.unwrap_or_default(),
        ephemeral: key.ephemeral,
        allow_extra_dns_labels: key.allow_extra_dns_labels,
        key: plaintext,
        reissue,
    })
}

fn setup_key_matches_spec(key: &api::SetupKey, spec: &EnsureSetupKeyInput) -> bool {
    key.key_type.as_deref() == Some(&spec.key_type)
        && key.usage_limit == Some(spec.usage_limit)
        && string_set_matches(
            key.auto_groups.as_deref().unwrap_or_default(),
            &spec.auto_groups,
        )
        && spec
            .ephemeral
            .is_none_or(|value| key.ephemeral == Some(value))
        && spec
            .allow_extra_dns_labels
            .is_none_or(|value| key.allow_extra_dns_labels == Some(value))
}

fn setup_key_reissue_required(
    matches: &[api::SetupKey],
    mutable_plaintext_present: bool,
    spec: &EnsureSetupKeyInput,
) -> bool {
    let valid = matches
        .iter()
        .filter(|key| key.valid == Some(true) && key.revoked != Some(true))
        .collect::<Vec<_>>();
    valid.len() != 1 || !mutable_plaintext_present || !setup_key_matches_spec(valid[0], spec)
}
async fn create_setup_key(
    client: &api::NetBirdClient,
    spec: &EnsureSetupKeyInput,
) -> ResourceResult<EnsureSetupKeyOutput> {
    let resolved = resolved_setup_key_input(client, spec).await?;
    let clear = client
        .create_setup_key(&resolved.to_request())
        .await
        .map_err(ResourceError::provider)?;
    let plaintext = clear
        .setup_key
        .key
        .clone()
        .filter(|key| !key.is_empty())
        .ok_or_else(|| {
            ResourceError::provider(
                "NetBird did not return plaintext for a newly-created setup key",
            )
        })?;
    setup_key_state(clear.setup_key, Some(plaintext), false).ok_or_else(|| missing_id("setup key"))
}
#[derive(Clone)]
pub struct SetupKeyResource {
    source: NetBirdClientSource,
}
#[async_trait]
impl Resource for SetupKeyResource {
    type Spec = EnsureSetupKeyInput;
    type State = EnsureSetupKeyOutput;
    fn kind(&self) -> &'static str {
        ENSURE_SETUP_KEY
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let stored =
            mutable_vault_field_present(ctx, &spec.mutable_vault_file, &spec.mutable_vault_field)
                .await?;
        let client = self.source.client(ctx).await?;
        let resolved = resolved_setup_key_input(&client, spec).await?;
        let matches = client
            .setup_keys()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|key| key.name.as_deref() == Some(&spec.name))
            .collect::<Vec<_>>();
        let valid = matches
            .iter()
            .filter(|key| key.valid == Some(true) && key.revoked != Some(true))
            .cloned()
            .collect::<Vec<_>>();
        match valid.as_slice() {
            [key] if !setup_key_reissue_required(&matches, stored, &resolved) => {
                setup_key_state(key.clone(), None, false)
                    .map(Some)
                    .ok_or_else(|| missing_id("setup key"))
            }
            _ if matches.is_empty() => Ok(None),
            _ => {
                let key = valid
                    .first()
                    .or(matches.first())
                    .expect("not empty")
                    .clone();
                setup_key_state(key, None, true)
                    .map(Some)
                    .ok_or_else(|| missing_id("setup key"))
            }
        }
    }
    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        create_setup_key(&client, spec).await
    }
    fn diff(&self, _spec: &Self::Spec, current: &Self::State) -> Drift {
        current
            .reissue
            .then_some(Drift::Drifted(
                "key invalid or mutable-vault plaintext missing".into(),
            ))
            .unwrap_or(Drift::InSync)
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        _current: Self::State,
    ) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        for key in client
            .setup_keys()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|key| key.name.as_deref() == Some(&spec.name))
        {
            let id = key.id.ok_or_else(|| missing_id("setup key"))?;
            client
                .update_setup_key(
                    &id,
                    &api::SetupKeyRequest {
                        revoked: true,
                        auto_groups: key.auto_groups.unwrap_or_default(),
                    },
                )
                .await
                .map_err(ResourceError::provider)?;
        }
        create_setup_key(&client, spec).await
    }
}

pub const ENSURE_REVERSE_PROXY_TOKEN: &str = "netbird.ensure_reverse_proxy_token";
pub type EnsureReverseProxyToken = EnsureResource<ReverseProxyTokenResource>;
pub fn ensure_reverse_proxy_token(source: NetBirdClientSource) -> EnsureReverseProxyToken {
    EnsureResource::new(ReverseProxyTokenResource { source })
}

/// Desired BYOP proxy token and the mutable-vault field that persists the
/// one-time plaintext. Existing API tokens without that field are deleted and
/// replaced instead of leaving an unrecoverable token behind.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureReverseProxyTokenInput {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
    pub mutable_vault_file: String,
    pub mutable_vault_field: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct EnsureReverseProxyTokenOutput {
    pub id: String,
    pub name: String,
    pub revoked: bool,
    /// Present only in the creation capture. JSON pointer: `/plain_token`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plain_token: Option<String>,
    #[serde(default)]
    reissue: bool,
}
impl std::fmt::Debug for EnsureReverseProxyTokenOutput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EnsureReverseProxyTokenOutput")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("revoked", &self.revoked)
            .field(
                "plain_token",
                &self.plain_token.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}
fn reverse_proxy_token_state(
    token: api::ReverseProxyToken,
    plaintext: Option<String>,
    reissue: bool,
) -> Option<EnsureReverseProxyTokenOutput> {
    Some(EnsureReverseProxyTokenOutput {
        id: token.id?,
        name: token.name?,
        revoked: token.revoked.unwrap_or(false),
        plain_token: plaintext,
        reissue,
    })
}
async fn create_reverse_proxy_token(
    client: &api::NetBirdClient,
    spec: &EnsureReverseProxyTokenInput,
) -> ResourceResult<EnsureReverseProxyTokenOutput> {
    let created = client
        .create_reverse_proxy_token(&api::CreateReverseProxyTokenRequest {
            name: spec.name.clone(),
            expires_in: spec.expires_in,
        })
        .await
        .map_err(ResourceError::provider)?;
    if created.plain_token.is_empty() {
        return Err(ResourceError::provider(
            "NetBird did not return plaintext for a newly-created reverse-proxy token",
        ));
    }
    reverse_proxy_token_state(created.token, Some(created.plain_token), false)
        .ok_or_else(|| missing_id("reverse-proxy token"))
}
#[derive(Clone)]
pub struct ReverseProxyTokenResource {
    source: NetBirdClientSource,
}
#[async_trait]
impl Resource for ReverseProxyTokenResource {
    type Spec = EnsureReverseProxyTokenInput;
    type State = EnsureReverseProxyTokenOutput;
    fn kind(&self) -> &'static str {
        ENSURE_REVERSE_PROXY_TOKEN
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let stored =
            mutable_vault_field_present(ctx, &spec.mutable_vault_file, &spec.mutable_vault_field)
                .await?;
        let matches = self
            .source
            .client(ctx)
            .await?
            .reverse_proxy_tokens()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|token| token.name.as_deref() == Some(&spec.name))
            .collect::<Vec<_>>();
        let active = matches
            .iter()
            .filter(|token| token.revoked != Some(true))
            .cloned()
            .collect::<Vec<_>>();
        match active.as_slice() {
            [token] if stored => reverse_proxy_token_state(token.clone(), None, false)
                .map(Some)
                .ok_or_else(|| missing_id("reverse-proxy token")),
            _ if matches.is_empty() => Ok(None),
            _ => {
                let token = active
                    .first()
                    .or(matches.first())
                    .expect("not empty")
                    .clone();
                reverse_proxy_token_state(token, None, true)
                    .map(Some)
                    .ok_or_else(|| missing_id("reverse-proxy token"))
            }
        }
    }
    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        create_reverse_proxy_token(&client, spec).await
    }
    fn diff(&self, _spec: &Self::Spec, current: &Self::State) -> Drift {
        current
            .reissue
            .then_some(Drift::Drifted(
                "token revoked or mutable-vault plaintext missing".into(),
            ))
            .unwrap_or(Drift::InSync)
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        _current: Self::State,
    ) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        for token in client
            .reverse_proxy_tokens()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|token| token.name.as_deref() == Some(&spec.name))
        {
            let id = token.id.ok_or_else(|| missing_id("reverse-proxy token"))?;
            client
                .delete_reverse_proxy_token(&id)
                .await
                .map_err(ResourceError::provider)?;
        }
        create_reverse_proxy_token(&client, spec).await
    }
}

// Reverse proxy custom domains
pub const ENSURE_REVERSE_PROXY_DOMAIN: &str = "netbird.ensure_reverse_proxy_domain";
pub type EnsureReverseProxyDomain = EnsureResource<ReverseProxyDomainResource>;
pub fn ensure_reverse_proxy_domain(source: NetBirdClientSource) -> EnsureReverseProxyDomain {
    EnsureResource::new(ReverseProxyDomainResource { source })
}

/// A custom domain attached to an already-connected BYOP cluster. NetBird does
/// not expose an update endpoint for domains, so `domain` and `target_cluster`
/// are immutable after creation.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureReverseProxyDomainInput {
    pub domain: String,
    pub target_cluster: String,
    #[serde(default = "yes")]
    pub validate: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureReverseProxyDomainOutput {
    pub id: String,
    pub domain: String,
    pub target_cluster: String,
    pub validated: bool,
}
impl EnsureReverseProxyDomainInput {
    fn to_request(&self) -> api::ReverseProxyDomainRequest {
        api::ReverseProxyDomainRequest {
            domain: self.domain.clone(),
            target_cluster: self.target_cluster.clone(),
        }
    }
}
fn reverse_proxy_domain_state(
    domain: api::ReverseProxyDomain,
) -> Option<EnsureReverseProxyDomainOutput> {
    Some(EnsureReverseProxyDomainOutput {
        id: domain.id?,
        domain: domain.domain?,
        target_cluster: domain.target_cluster?,
        validated: domain.validated.unwrap_or(false),
    })
}
async fn require_online_reverse_proxy_cluster(
    client: &api::NetBirdClient,
    address: &str,
) -> ResourceResult<()> {
    let matches = client
        .reverse_proxy_clusters()
        .await
        .map_err(ResourceError::provider)?
        .into_iter()
        .filter(|cluster| cluster.address.as_deref() == Some(address))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [cluster] if cluster.online == Some(true) => Ok(()),
        [.., _, _] => Err(ResourceError::provider(format!(
            "more than one NetBird reverse-proxy cluster has address {address:?}"
        ))),
        [_] => Err(ResourceError::provider(format!(
            "NetBird reverse-proxy cluster {address:?} is not online yet"
        ))),
        [] => Err(ResourceError::provider(format!(
            "no NetBird reverse-proxy cluster has address {address:?}; start the BYOP proxy first"
        ))),
    }
}
#[derive(Clone)]
pub struct ReverseProxyDomainResource {
    source: NetBirdClientSource,
}
#[async_trait]
impl Resource for ReverseProxyDomainResource {
    type Spec = EnsureReverseProxyDomainInput;
    type State = EnsureReverseProxyDomainOutput;
    fn kind(&self) -> &'static str {
        ENSURE_REVERSE_PROXY_DOMAIN
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let matches = self
            .source
            .client(ctx)
            .await?
            .reverse_proxy_domains()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|domain| domain.domain.as_deref() == Some(&spec.domain))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [domain] => reverse_proxy_domain_state(domain.clone())
                .map(Some)
                .ok_or_else(|| missing_id("reverse-proxy domain")),
            _ => Err(ResourceError::provider(format!(
                "more than one NetBird reverse-proxy domain is named {:?}",
                spec.domain
            ))),
        }
    }
    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        if spec.domain.trim().is_empty() || spec.target_cluster.trim().is_empty() {
            return Err(ResourceError::provider(
                "reverse-proxy domain and target_cluster must not be empty",
            ));
        }
        let client = self.source.client(ctx).await?;
        require_online_reverse_proxy_cluster(&client, &spec.target_cluster).await?;
        let state = reverse_proxy_domain_state(
            client
                .create_reverse_proxy_domain(&spec.to_request())
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("reverse-proxy domain"))?;
        if spec.validate {
            client
                .validate_reverse_proxy_domain(&state.id)
                .await
                .map_err(ResourceError::provider)?;
        }
        Ok(state)
    }
    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        drift(
            [
                (
                    current.target_cluster != spec.target_cluster,
                    "target_cluster",
                ),
                (spec.validate && !current.validated, "validation"),
            ]
            .into_iter()
            .filter_map(|(bad, part)| bad.then_some(part.into()))
            .collect(),
        )
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        if current.target_cluster != spec.target_cluster {
            return Err(ResourceError::provider(
                "NetBird does not support moving a custom reverse-proxy domain to another cluster; delete and recreate it deliberately",
            ));
        }
        if spec.validate && !current.validated {
            self.source
                .client(ctx)
                .await?
                .validate_reverse_proxy_domain(&current.id)
                .await
                .map_err(ResourceError::provider)?;
        }
        Ok(current)
    }
    async fn delete(&self, ctx: &ResourceCtx, state: &Self::State) -> ResourceResult<()> {
        self.source
            .client(ctx)
            .await?
            .delete_reverse_proxy_domain(&state.id)
            .await
            .map_err(ResourceError::provider)
    }
}

// Reverse proxy services
pub const ENSURE_REVERSE_PROXY_SERVICE: &str = "netbird.ensure_reverse_proxy_service";
pub type EnsureReverseProxyService = EnsureResource<ReverseProxyServiceResource>;
pub fn ensure_reverse_proxy_service(source: NetBirdClientSource) -> EnsureReverseProxyService {
    EnsureResource::new(ReverseProxyServiceResource { source })
}

/// A service backend. Set exactly one of `target_id`, `peer_name`, or the
/// network-resource selector. The latter two are resolved when the node runs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReverseProxyServiceTargetInput {
    #[serde(default)]
    pub target_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_name: Option<String>,
    #[serde(default)]
    pub network_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_name: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub port: u16,
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<ReverseProxyServiceTargetOptionsInput>,
}
impl Default for ReverseProxyServiceTargetInput {
    fn default() -> Self {
        Self {
            target_id: String::new(),
            target_type: None,
            peer_name: None,
            network_id: String::new(),
            network_name: None,
            resource_name: None,
            enabled: true,
            host: None,
            path: None,
            port: 0,
            protocol: String::new(),
            options: None,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReverseProxyServiceTargetOptionsInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_headers: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_upstream: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_rewrite: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_protocol: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_timeout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_idle_timeout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_tls_verify: Option<bool>,
}
impl From<ReverseProxyServiceTargetOptionsInput> for api::ReverseProxyServiceTargetOptions {
    fn from(value: ReverseProxyServiceTargetOptionsInput) -> Self {
        Self {
            custom_headers: value.custom_headers,
            direct_upstream: value.direct_upstream,
            path_rewrite: value.path_rewrite,
            proxy_protocol: value.proxy_protocol,
            request_timeout: value.request_timeout,
            session_idle_timeout: value.session_idle_timeout,
            skip_tls_verify: value.skip_tls_verify,
        }
    }
}
impl ReverseProxyServiceTargetInput {
    fn to_api(&self) -> api::ReverseProxyServiceTarget {
        api::ReverseProxyServiceTarget {
            enabled: self.enabled,
            host: self.host.clone(),
            path: self.path.clone(),
            port: self.port,
            protocol: self.protocol.clone(),
            target_id: self.target_id.clone(),
            target_type: self.target_type.clone().unwrap_or_default(),
            options: self.options.clone().map(Into::into),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnsureReverseProxyServiceInput {
    pub name: String,
    pub domain: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "http")]
    pub mode: String,
    #[serde(default)]
    pub targets: Vec<ReverseProxyServiceTargetInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_groups: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_group_names: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<api::ReverseProxyServiceAuth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pass_host_header: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewrite_redirects: Option<bool>,
}
impl Default for EnsureReverseProxyServiceInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            domain: String::new(),
            enabled: true,
            mode: http(),
            targets: Vec::new(),
            access_groups: None,
            access_group_names: None,
            auth: None,
            listen_port: None,
            pass_host_header: None,
            private: None,
            rewrite_redirects: None,
        }
    }
}
fn http() -> String {
    "http".into()
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureReverseProxyServiceOutput {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub enabled: bool,
    pub mode: Option<String>,
    pub targets: Vec<api::ReverseProxyServiceTarget>,
    pub access_groups: Option<Vec<String>>,
    pub auth: Option<api::ReverseProxyServiceAuth>,
    pub listen_port: Option<u16>,
    pub pass_host_header: Option<bool>,
    pub private: Option<bool>,
    pub rewrite_redirects: Option<bool>,
    pub terminated: bool,
}
fn reverse_proxy_service_state(
    service: api::ReverseProxyService,
) -> Option<EnsureReverseProxyServiceOutput> {
    Some(EnsureReverseProxyServiceOutput {
        id: service.id?,
        name: service.name?,
        domain: service.domain?,
        enabled: service.enabled.unwrap_or(false),
        mode: service.mode,
        targets: service.targets.unwrap_or_default(),
        access_groups: service.access_groups,
        auth: service.auth,
        listen_port: service.listen_port,
        pass_host_header: service.pass_host_header,
        private: service.private,
        rewrite_redirects: service.rewrite_redirects,
        terminated: service.terminated.unwrap_or(false),
    })
}
async fn resolve_service_target(
    client: &api::NetBirdClient,
    input: &ReverseProxyServiceTargetInput,
) -> ResourceResult<ReverseProxyServiceTargetInput> {
    let resource_selector = input.resource_name.is_some();
    let selectors = usize::from(!input.target_id.trim().is_empty())
        + usize::from(input.peer_name.is_some())
        + usize::from(resource_selector);
    if selectors != 1 {
        return Err(ResourceError::provider(
            "reverse-proxy target must set exactly one of target_id, peer_name, or resource_name",
        ));
    }
    let mut resolved = input.clone();
    if let Some(peer_name) = &input.peer_name {
        let matches = client
            .peers(Some(peer_name), None)
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|peer| peer.name.as_deref() == Some(peer_name))
            .collect::<Vec<_>>();
        let peer = match matches.as_slice() {
            [peer] => peer,
            [] => {
                return Err(ResourceError::provider(format!(
                    "no NetBird peer named {peer_name:?}"
                )))
            }
            _ => {
                return Err(ResourceError::provider(format!(
                    "more than one NetBird peer is named {peer_name:?}"
                )))
            }
        };
        resolved.target_id = peer.id.clone().ok_or_else(|| missing_id("peer"))?;
        resolved.target_type = Some("peer".into());
    } else if let Some(resource_name) = &input.resource_name {
        let network_id =
            resolve_network_id(client, &input.network_id, input.network_name.as_deref()).await?;
        let matches = client
            .network_resources(&network_id)
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|resource| resource.name.as_deref() == Some(resource_name))
            .collect::<Vec<_>>();
        let resource = match matches.as_slice() {
            [resource] => resource,
            [] => {
                return Err(ResourceError::provider(format!(
                    "no NetBird network resource named {resource_name:?}"
                )))
            }
            _ => {
                return Err(ResourceError::provider(format!(
                    "more than one NetBird network resource is named {resource_name:?}"
                )))
            }
        };
        resolved.network_id = network_id;
        resolved.target_id = resource
            .id
            .clone()
            .ok_or_else(|| missing_id("network resource"))?;
        let actual_type = resource.resource_type.clone().ok_or_else(|| {
            ResourceError::provider("NetBird returned a network resource without a type")
        })?;
        if let Some(expected_type) = &input.target_type {
            if expected_type != &actual_type {
                return Err(ResourceError::provider(format!(
                    "reverse-proxy target {resource_name:?} has type {actual_type:?}, not {expected_type:?}"
                )));
            }
        }
        resolved.target_type = Some(actual_type);
    } else if input.target_type.as_deref().is_none_or(str::is_empty) {
        return Err(ResourceError::provider(
            "reverse-proxy target_id requires target_type",
        ));
    }
    Ok(resolved)
}
async fn resolved_reverse_proxy_service_input(
    client: &api::NetBirdClient,
    input: &EnsureReverseProxyServiceInput,
) -> ResourceResult<EnsureReverseProxyServiceInput> {
    if input.access_groups.is_some() && input.access_group_names.is_some() {
        return Err(ResourceError::provider(
            "reverse-proxy service must use access_groups or access_group_names, not both",
        ));
    }
    let mut resolved = input.clone();
    resolved.targets.clear();
    for target in &input.targets {
        resolved
            .targets
            .push(resolve_service_target(client, target).await?);
    }
    if let Some(names) = &input.access_group_names {
        resolved.access_groups = Some(resolve_group_names(client, names).await?);
    }
    resolved.access_group_names = None;
    Ok(resolved)
}
impl EnsureReverseProxyServiceInput {
    fn to_request(&self) -> api::ReverseProxyServiceRequest {
        api::ReverseProxyServiceRequest {
            access_groups: self.access_groups.clone(),
            auth: self.auth.clone(),
            domain: self.domain.clone(),
            enabled: self.enabled,
            listen_port: self.listen_port,
            mode: Some(self.mode.clone()),
            name: self.name.clone(),
            pass_host_header: self.pass_host_header,
            private: self.private,
            rewrite_redirects: self.rewrite_redirects,
            targets: Some(
                self.targets
                    .iter()
                    .map(ReverseProxyServiceTargetInput::to_api)
                    .collect(),
            ),
        }
    }
}
fn service_targets_match(
    current: &[api::ReverseProxyServiceTarget],
    desired: &[ReverseProxyServiceTargetInput],
) -> bool {
    current.len() == desired.len()
        && current.iter().zip(desired).all(|(current, desired)| {
            current.enabled == desired.enabled
                && current.host == desired.host
                && current.path == desired.path
                && current.port == desired.port
                && current.protocol == desired.protocol
                && current.options == desired.options.clone().map(Into::into)
                && (desired.peer_name.is_some()
                    || desired.resource_name.is_some()
                    || (current.target_id == desired.target_id
                        && current.target_type == desired.target_type.clone().unwrap_or_default()))
        })
}
#[derive(Clone)]
pub struct ReverseProxyServiceResource {
    source: NetBirdClientSource,
}
#[async_trait]
impl Resource for ReverseProxyServiceResource {
    type Spec = EnsureReverseProxyServiceInput;
    type State = EnsureReverseProxyServiceOutput;
    fn kind(&self) -> &'static str {
        ENSURE_REVERSE_PROXY_SERVICE
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let matches = self
            .source
            .client(ctx)
            .await?
            .reverse_proxy_services()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .filter(|service| service.name.as_deref() == Some(&spec.name))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [service] => reverse_proxy_service_state(service.clone())
                .map(Some)
                .ok_or_else(|| missing_id("reverse-proxy service")),
            _ => Err(ResourceError::provider(format!(
                "more than one NetBird reverse-proxy service is named {:?}",
                spec.name
            ))),
        }
    }
    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let resolved = resolved_reverse_proxy_service_input(&client, spec).await?;
        reverse_proxy_service_state(
            client
                .create_reverse_proxy_service(&resolved.to_request())
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("reverse-proxy service"))
    }
    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let access_groups_match = spec.access_group_names.as_ref().is_some_and(|_| true)
            || optional_string_set_matches(
                current.access_groups.as_ref(),
                spec.access_groups.as_ref(),
            );
        drift(
            [
                (current.domain != spec.domain, "domain"),
                (current.enabled != spec.enabled, "enabled"),
                (current.mode.as_deref() != Some(&spec.mode), "mode"),
                (
                    !service_targets_match(&current.targets, &spec.targets),
                    "targets",
                ),
                (!access_groups_match, "access_groups"),
                (current.auth != spec.auth, "auth"),
                (current.listen_port != spec.listen_port, "listen_port"),
                (
                    current.pass_host_header != spec.pass_host_header,
                    "pass_host_header",
                ),
                (current.private != spec.private, "private"),
                (
                    current.rewrite_redirects != spec.rewrite_redirects,
                    "rewrite_redirects",
                ),
            ]
            .into_iter()
            .filter_map(|(bad, part)| bad.then_some(part.into()))
            .collect(),
        )
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        if current.terminated {
            return Err(ResourceError::provider(
                "NetBird will not update a terminated reverse-proxy service",
            ));
        }
        let client = self.source.client(ctx).await?;
        let resolved = resolved_reverse_proxy_service_input(&client, spec).await?;
        reverse_proxy_service_state(
            client
                .update_reverse_proxy_service(&current.id, &resolved.to_request())
                .await
                .map_err(ResourceError::provider)?,
        )
        .ok_or_else(|| missing_id("reverse-proxy service"))
    }
    async fn delete(&self, ctx: &ResourceCtx, state: &Self::State) -> ResourceResult<()> {
        self.source
            .client(ctx)
            .await?
            .delete_reverse_proxy_service(&state.id)
            .await
            .map_err(ResourceError::provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use axum::{
        extract::{Path, State},
        http::StatusCode,
        routing::{get, put},
        Json, Router,
    };
    use infrazeug_native::{NodeCtx, SecretSource};
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    #[test]
    fn network_resource_request_defaults_to_enabled() {
        let request = EnsureNetworkResourceInput {
            name: "home-dns".into(),
            address: "192.168.26.1/32".into(),
            ..Default::default()
        }
        .to_request();

        assert_eq!(serde_json::to_value(request).unwrap()["enabled"], true);
    }

    #[derive(Default)]
    struct IdentityProviderApi {
        provider: Mutex<Option<Value>>,
        requests: Mutex<Vec<Value>>,
    }

    #[derive(Default)]
    struct RouterApi {
        routers: Mutex<Vec<Value>>,
        update_requests: Mutex<Vec<Value>>,
        deleted_ids: Mutex<Vec<String>>,
    }

    async fn list_router_peers() -> Json<Value> {
        Json(json!([{"id": "peer-edge", "name": "edge-vps"}]))
    }

    async fn list_network_routers(State(api): State<Arc<RouterApi>>) -> Json<Value> {
        Json(Value::Array(api.routers.lock().unwrap().clone()))
    }

    async fn update_network_router(
        Path((_network_id, router_id)): Path<(String, String)>,
        State(api): State<Arc<RouterApi>>,
        Json(request): Json<Value>,
    ) -> Json<Value> {
        api.update_requests.lock().unwrap().push(request.clone());
        let mut routers = api.routers.lock().unwrap();
        let router = routers
            .iter_mut()
            .find(|router| router["id"] == router_id)
            .expect("router to update");
        router["peer"] = request["peer"].clone();
        router["peer_groups"] = request["peer_groups"].clone();
        router["metric"] = request["metric"].clone();
        router["masquerade"] = request["masquerade"].clone();
        router["enabled"] = request["enabled"].clone();
        Json(router.clone())
    }

    async fn delete_network_router(
        Path((_network_id, router_id)): Path<(String, String)>,
        State(api): State<Arc<RouterApi>>,
    ) -> StatusCode {
        api.deleted_ids.lock().unwrap().push(router_id.clone());
        api.routers
            .lock()
            .unwrap()
            .retain(|router| router["id"] != router_id);
        StatusCode::NO_CONTENT
    }

    #[tokio::test]
    async fn router_peer_name_resolves_to_an_exact_api_peer_id() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/api/peers", get(list_router_peers)),
            )
            .await
            .unwrap();
        });
        let client = api::NetBirdClient::new(
            api::NetBirdConfig::new(api::Auth::oauth_token("test"))
                .with_host(format!("http://{address}")),
        );
        let input = EnsureNetworkRouterInput {
            network_id: "network-1".into(),
            peer_name: Some("edge-vps".into()),
            metric: 100,
            ..Default::default()
        };

        let resolved = resolved_router_input(&client, &input).await.unwrap();
        assert_eq!(resolved.peer.as_deref(), Some("peer-edge"));
        assert!(resolved.peer_name.is_none());
        assert!(resolved.peer_groups.is_none());
        assert_eq!(resolved.to_request().peer.as_deref(), Some("peer-edge"));
        server.abort();
    }

    #[tokio::test]
    async fn duplicate_group_routers_reconcile_to_one_deterministic_router() {
        let api = Arc::new(RouterApi {
            routers: Mutex::new(vec![
                json!({
                    "id": "router-b",
                    "peer_groups": ["group-home"],
                    "metric": 100,
                    "masquerade": true,
                    "enabled": true,
                }),
                json!({
                    "id": "router-a",
                    "peer_groups": ["group-home"],
                    "metric": 100,
                    "masquerade": true,
                    "enabled": true,
                }),
            ]),
            ..Default::default()
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server_api = Arc::clone(&api);
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route(
                        "/api/networks/{network_id}/routers",
                        get(list_network_routers),
                    )
                    .route(
                        "/api/networks/{network_id}/routers/{router_id}",
                        put(update_network_router).delete(delete_network_router),
                    )
                    .with_state(server_api),
            )
            .await
            .unwrap();
        });
        let client = api::NetBirdClient::new(
            api::NetBirdConfig::new(api::Auth::oauth_token("test"))
                .with_host(format!("http://{address}")),
        );
        let resource = NetworkRouterResource {
            source: NetBirdClientSource::ready(client),
        };
        let spec = EnsureNetworkRouterInput {
            network_id: "network-1".into(),
            peer_groups: Some(vec!["group-home".into()]),
            metric: 100,
            masquerade: true,
            enabled: true,
            ..Default::default()
        };
        let node = NodeCtx::new(Uuid::nil(), Uuid::nil());
        let ctx = ResourceCtx::from(&node);

        let observed = resource.observe(&ctx, &spec).await.unwrap().unwrap();
        assert_eq!(observed.id, "router-a");
        assert_eq!(observed.duplicate_ids, vec!["router-b"]);
        assert!(matches!(resource.diff(&spec, &observed), Drift::Drifted(_)));

        let reconciled = resource.reconcile(&ctx, &spec, observed).await.unwrap();
        assert!(reconciled.duplicate_ids.is_empty());
        assert_eq!(api.deleted_ids.lock().unwrap().as_slice(), ["router-b"]);
        assert_eq!(api.routers.lock().unwrap().len(), 1);
        assert_eq!(
            api.update_requests.lock().unwrap()[0]["peer_groups"],
            json!(["group-home"])
        );
        server.abort();
    }

    async fn list_identity_providers(State(api): State<Arc<IdentityProviderApi>>) -> Json<Value> {
        Json(Value::Array(
            api.provider.lock().unwrap().clone().into_iter().collect(),
        ))
    }

    async fn create_identity_provider(
        State(api): State<Arc<IdentityProviderApi>>,
        Json(request): Json<Value>,
    ) -> Json<Value> {
        api.requests.lock().unwrap().push(request.clone());
        let response = json!({
            "id": "idp-1",
            "name": request["name"],
            "type": request["type"],
            "issuer": request["issuer"],
            "client_id": request["client_id"],
        });
        *api.provider.lock().unwrap() = Some(response.clone());
        Json(response)
    }

    async fn update_identity_provider(
        State(api): State<Arc<IdentityProviderApi>>,
        Json(request): Json<Value>,
    ) -> Json<Value> {
        create_identity_provider(State(api), Json(request)).await
    }

    struct IdentityProviderSecret;

    #[async_trait]
    impl SecretSource for IdentityProviderSecret {
        async fn read_field(&self, _file: &str, _field: &str) -> infrazeug_native::Result<Vec<u8>> {
            Err(infrazeug_native::NativeError::other(
                "static vault must not be used for generated Keycloak secret",
            ))
        }

        fn has_mutable_vault(&self) -> bool {
            true
        }

        async fn read_mutable_field(
            &self,
            file: &str,
            field: &str,
        ) -> infrazeug_native::Result<Vec<u8>> {
            assert_eq!(
                (file, field),
                ("keycloak/netbird-generated.vault", "client_secret")
            );
            Ok(b"keycloak-client-secret".to_vec())
        }
    }

    #[tokio::test]
    async fn identity_provider_creates_observes_and_updates_without_secret_drift() {
        let api = Arc::new(IdentityProviderApi::default());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server_api = Arc::clone(&api);
        let server = tokio::spawn(async move {
            let router = Router::new()
                .route(
                    "/api/identity-providers",
                    get(list_identity_providers).post(create_identity_provider),
                )
                .route(
                    "/api/identity-providers/{id}",
                    put(update_identity_provider),
                )
                .with_state(server_api);
            axum::serve(listener, router).await.unwrap();
        });
        let client = api::NetBirdClient::new(
            api::NetBirdConfig::new(api::Auth::oauth_token("test"))
                .with_host(format!("http://{address}")),
        );
        let resource = IdentityProviderResource {
            source: NetBirdClientSource::ready(client),
        };
        let spec = EnsureIdentityProviderInput {
            name: "Keycloak".into(),
            provider_type: "oidc".into(),
            issuer: "https://keycloak.example/realms/netbird".into(),
            client_id: "netbird".into(),
            client_secret_source: Some(IdentityProviderClientSecretSource::MutableVault {
                file: "keycloak/netbird-generated.vault".into(),
                field: "client_secret".into(),
            }),
        };
        let node = NodeCtx::new(Uuid::nil(), Uuid::nil())
            .with_secrets(Some(
                Arc::new(IdentityProviderSecret) as Arc<dyn SecretSource>
            ));
        let ctx = ResourceCtx::from(&node);

        assert!(resource.observe(&ctx, &spec).await.unwrap().is_none());
        let created = resource.create(&ctx, &spec).await.unwrap();
        assert_eq!(created.id, "idp-1");
        assert_eq!(created.client_id, "netbird");
        assert!(!serde_json::to_string(&created)
            .unwrap()
            .contains("keycloak-client-secret"));
        assert_eq!(
            api.requests.lock().unwrap()[0]["client_secret"],
            "keycloak-client-secret"
        );

        let in_sync = resource.observe(&ctx, &spec).await.unwrap().unwrap();
        assert_eq!(resource.diff(&spec, &in_sync), Drift::InSync);
        assert_eq!(api.requests.lock().unwrap().len(), 1);

        api.provider.lock().unwrap().as_mut().unwrap()["issuer"] = json!("https://old.example");
        let observed = resource.observe(&ctx, &spec).await.unwrap().unwrap();
        assert!(matches!(resource.diff(&spec, &observed), Drift::Drifted(_)));
        let updated = resource.reconcile(&ctx, &spec, observed).await.unwrap();
        assert_eq!(updated.issuer, spec.issuer);
        assert_eq!(api.requests.lock().unwrap().len(), 2);
        assert_eq!(
            api.requests.lock().unwrap()[1]["client_secret"],
            "keycloak-client-secret"
        );

        let request = identity_provider_request(&ctx, &spec).await.unwrap();
        assert!(!format!("{request:?}").contains("keycloak-client-secret"));
        server.abort();
    }

    #[test]
    fn group_request_preserves_references() {
        let input = EnsureGroupInput {
            name: "ops".into(),
            peers: Some(vec!["peer-a".into()]),
            peer_names: None,
            resources: Some(vec![ResourceRef {
                id: "resource-a".into(),
                resource_type: "host".into(),
            }]),
        };
        let request = input.to_request();
        assert_eq!(request.name, "ops");
        assert_eq!(request.resources.unwrap()[0].resource_type, "host");
    }
    #[test]
    fn route_diff_detects_mutable_settings() {
        let spec = EnsureRouteInput {
            network_id: "n".into(),
            description: "corp".into(),
            groups: vec!["g".into()],
            ..Default::default()
        };
        let current = EnsureRouteOutput {
            id: "r".into(),
            network_id: "n".into(),
            description: "corp".into(),
            enabled: true,
            peer: None,
            peer_groups: None,
            network: None,
            domains: None,
            metric: 0,
            masquerade: false,
            groups: vec![],
            keep_route: false,
            access_control_groups: vec![],
            skip_auto_apply: false,
        };
        assert!(matches!(
            RouteResource {
                source: NetBirdClientSource::vault("x")
            }
            .diff(&spec, &current),
            Drift::Drifted(_)
        ));
    }

    #[test]
    fn set_valued_fields_do_not_drift_on_api_order() {
        let source = NetBirdClientSource::vault("x");
        let group = GroupResource {
            source: source.clone(),
        };
        let group_spec = EnsureGroupInput {
            name: "ops".into(),
            peers: Some(vec!["peer-a".into(), "peer-b".into()]),
            ..Default::default()
        };
        let group_state = EnsureGroupOutput {
            id: "group-1".into(),
            name: "ops".into(),
            peers: Some(vec!["peer-b".into(), "peer-a".into()]),
            peer_names: None,
            resources: Some(vec![]),
        };
        assert_eq!(group.diff(&group_spec, &group_state), Drift::InSync);

        let dns = DnsSettingsResource { source };
        let dns_spec = EnsureDnsSettingsInput {
            disabled_management_groups: vec!["a".into(), "b".into()],
        };
        let dns_state = EnsureDnsSettingsOutput {
            disabled_management_groups: vec!["b".into(), "a".into()],
        };
        assert_eq!(dns.diff(&dns_spec, &dns_state), Drift::InSync);
    }

    #[test]
    fn invalid_route_and_router_inputs_fail_before_an_api_call() {
        let route = EnsureRouteInput {
            network_id: "office".into(),
            description: "office".into(),
            peer: Some("peer-a".into()),
            peer_groups: Some(vec!["routers".into()]),
            network: Some("192.0.2.0/24".into()),
            ..Default::default()
        };
        assert!(validate_route(&route).is_err());

        let router = EnsureNetworkRouterInput {
            network_name: Some("office".into()),
            ..Default::default()
        };
        assert!(validate_router(&router).is_err());

        let ambiguous_router = EnsureNetworkRouterInput {
            peer: Some("peer-id".into()),
            peer_name: Some("edge-vps".into()),
            metric: 100,
            ..Default::default()
        };
        assert!(validate_router(&ambiguous_router).is_err());
    }

    #[test]
    fn policy_rule_comparison_ignores_rule_and_group_order() {
        let left = vec![
            PolicyRuleInput {
                name: "ssh".into(),
                sources: vec!["b".into(), "a".into()],
                destinations: vec!["servers".into()],
                ..Default::default()
            },
            PolicyRuleInput {
                name: "dns".into(),
                destinations: vec!["resolvers".into()],
                ..Default::default()
            },
        ];
        let right = vec![
            left[1].clone(),
            PolicyRuleInput {
                id: Some("server-id".into()),
                sources: vec!["a".into(), "b".into()],
                ..left[0].clone()
            },
        ];
        assert_eq!(
            normalized_policy_rules(&left),
            normalized_policy_rules(&right)
        );
    }

    #[test]
    fn group_peer_names_are_compared_without_server_ids() {
        let spec = EnsureGroupInput {
            name: "edge".into(),
            peer_names: Some(vec!["edge-1".into(), "edge-2".into()]),
            ..Default::default()
        };
        let state = EnsureGroupOutput {
            id: "group-1".into(),
            name: "edge".into(),
            peers: Some(vec!["server-peer-2".into(), "server-peer-1".into()]),
            peer_names: Some(vec!["edge-2".into(), "edge-1".into()]),
            resources: None,
        };
        assert_eq!(
            GroupResource {
                source: NetBirdClientSource::vault("x"),
            }
            .diff(&spec, &state),
            Drift::InSync
        );
    }

    #[test]
    fn reverse_proxy_service_target_keeps_logical_selector_out_of_request() {
        let target = ReverseProxyServiceTargetInput {
            network_name: Some("k3s".into()),
            resource_name: Some("ingress".into()),
            target_id: "resource-id".into(),
            target_type: Some("host".into()),
            port: 443,
            protocol: "https".into(),
            ..Default::default()
        };
        let request = target.to_api();
        assert_eq!(request.target_id, "resource-id");
        assert_eq!(request.target_type, "host");
        assert_eq!(request.port, 443);
    }

    #[test]
    fn reverse_proxy_service_defaults_enable_services_and_targets() {
        let target = ReverseProxyServiceTargetInput::default();
        assert_eq!(serde_json::to_value(&target).unwrap()["enabled"], true);
        assert!(target.to_api().enabled);

        let service = EnsureReverseProxyServiceInput::default();
        assert_eq!(serde_json::to_value(&service).unwrap()["enabled"], true);
        assert!(service.to_request().enabled);
    }

    #[test]
    fn disabled_reverse_proxy_service_or_target_drifts_from_defaults() {
        let spec = EnsureReverseProxyServiceInput {
            name: "blog".into(),
            domain: "codefionn.eu".into(),
            targets: vec![ReverseProxyServiceTargetInput {
                target_id: "peer-1".into(),
                target_type: Some("peer".into()),
                port: 8443,
                protocol: "http".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let current = EnsureReverseProxyServiceOutput {
            id: "service-1".into(),
            name: spec.name.clone(),
            domain: spec.domain.clone(),
            enabled: false,
            mode: Some(spec.mode.clone()),
            targets: vec![api::ReverseProxyServiceTarget {
                enabled: false,
                target_id: "peer-1".into(),
                target_type: "peer".into(),
                port: 8443,
                protocol: "http".into(),
                ..Default::default()
            }],
            access_groups: None,
            auth: None,
            listen_port: None,
            pass_host_header: None,
            private: None,
            rewrite_redirects: None,
            terminated: false,
        };
        let resource = ReverseProxyServiceResource {
            source: NetBirdClientSource::vault("x"),
        };

        assert_eq!(
            resource.diff(&spec, &current),
            Drift::Drifted("enabled, targets".into())
        );
    }

    #[test]
    fn policy_group_names_match_returned_group_names() {
        let desired = PolicyRuleInput {
            name: "app-to-ingress".into(),
            source_group_names: Some(vec!["apps".into()]),
            destination_group_names: Some(vec!["ingress".into()]),
            ..Default::default()
        };
        let current = PolicyRuleInput {
            name: "app-to-ingress".into(),
            sources: vec!["group-apps".into()],
            source_group_names: Some(vec!["apps".into()]),
            destinations: vec!["group-ingress".into()],
            destination_group_names: Some(vec!["ingress".into()]),
            ..Default::default()
        };
        assert!(policy_rules_match(&[current], &[desired]));
    }

    #[test]
    fn policy_defaults_match_the_management_api_defaults() {
        let policy = EnsurePolicyInput {
            rules: vec![PolicyRuleInput {
                name: "allow-ingress".into(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let request = policy.to_request();
        let rule = &request.rules[0];
        assert!(request.enabled);
        assert!(rule.enabled);
        assert_eq!(rule.action, "accept");
        assert_eq!(rule.protocol, "all");
        assert!(!rule.bidirectional);
    }

    #[test]
    fn resource_backed_policy_rules_omit_conflicting_group_selectors() {
        let policy = EnsurePolicyInput {
            name: "k3s ingress".into(),
            rules: vec![PolicyRuleInput {
                name: "vpn-to-ingress".into(),
                sources: vec!["source-group-id".into()],
                source_resource: Some(PolicyNetworkResourceRef {
                    resource_id: "source-resource-id".into(),
                    resource_type: Some("host".into()),
                    ..Default::default()
                }),
                destinations: vec!["destination-group-id".into()],
                destination_resource: Some(PolicyNetworkResourceRef {
                    resource_id: "destination-resource-id".into(),
                    resource_type: Some("host".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        };

        let request = serde_json::to_value(policy.to_request()).unwrap();
        let rule = &request["rules"][0];
        assert!(rule.get("sources").is_none());
        assert!(rule.get("destinations").is_none());
        assert_eq!(rule["sourceResource"]["id"], "source-resource-id");
        assert_eq!(rule["destinationResource"]["id"], "destination-resource-id");
    }

    #[tokio::test]
    async fn nameserver_groups_resolve_names_and_compare_them_without_id_drift() {
        async fn groups() -> Json<Value> {
            Json(json!([{
                "id": "group-home",
                "name": "home-routing-peers"
            }]))
        }
        async fn group() -> Json<Value> {
            Json(json!({
                "id": "group-home",
                "name": "home-routing-peers"
            }))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/api/groups", get(groups))
                    .route("/api/groups/{id}", get(group)),
            )
            .await
            .unwrap();
        });
        let client = api::NetBirdClient::new(
            api::NetBirdConfig::new(api::Auth::oauth_token("test"))
                .with_host(format!("http://{address}")),
        );
        let spec = EnsureNameserverGroupInput {
            name: "cluster-dns".into(),
            groups: vec![],
            group_names: Some(vec!["home-routing-peers".into()]),
            ..Default::default()
        };

        let resolved = resolved_nameserver_group_input(&client, &spec)
            .await
            .unwrap();
        assert_eq!(resolved.groups, ["group-home"]);
        assert_eq!(resolved.group_names, None);
        assert_eq!(resolved.to_request().groups, ["group-home"]);

        let current = nameserver_group_state(
            &client,
            api::NameserverGroup {
                id: Some("dns-1".into()),
                name: Some("cluster-dns".into()),
                groups: Some(vec!["group-home".into()]),
                ..Default::default()
            },
            true,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(current.group_names, Some(vec!["home-routing-peers".into()]));
        assert_eq!(
            NameserverGroupResource {
                source: NetBirdClientSource::ready(client),
            }
            .diff(&spec, &current),
            Drift::InSync
        );
        server.abort();
    }

    #[test]
    fn nameserver_group_rejects_ids_and_names_together() {
        let spec = EnsureNameserverGroupInput {
            groups: vec!["group-id".into()],
            group_names: Some(vec!["home-routing-peers".into()]),
            ..Default::default()
        };
        assert!(validate_nameserver_group(&spec).is_err());
    }

    #[test]
    fn generated_credential_outputs_redact_plaintext_in_debug() {
        let setup = EnsureSetupKeyOutput {
            id: "key-id".into(),
            name: "edge".into(),
            key_type: "one-off".into(),
            valid: true,
            revoked: false,
            used_times: 0,
            usage_limit: 1,
            auto_groups: vec![],
            ephemeral: None,
            allow_extra_dns_labels: None,
            key: Some("setup-secret".into()),
            reissue: false,
        };
        let token = EnsureReverseProxyTokenOutput {
            id: "token-id".into(),
            name: "byop".into(),
            revoked: false,
            plain_token: Some("proxy-secret".into()),
            reissue: false,
        };
        assert!(!format!("{setup:?}").contains("setup-secret"));
        assert!(!format!("{token:?}").contains("proxy-secret"));
        assert_eq!(serde_json::to_value(setup).unwrap()["key"], "setup-secret");
        assert_eq!(
            serde_json::to_value(token).unwrap()["plain_token"],
            "proxy-secret"
        );
    }

    #[test]
    fn setup_key_request_keeps_mutable_vault_out_of_api_json() {
        let input = EnsureSetupKeyInput {
            name: "edge".into(),
            key_type: "one-off".into(),
            expires_in: 3_600,
            auto_groups: vec!["group-id".into()],
            auto_group_names: None,
            usage_limit: 1,
            ephemeral: None,
            allow_extra_dns_labels: None,
            mutable_vault_file: "netbird-generated.vault".into(),
            mutable_vault_field: "edge_setup_key".into(),
        };
        let value = serde_json::to_value(input.to_request()).unwrap();
        assert_eq!(value["auto_groups"][0], "group-id");
        assert!(value.get("mutable_vault_file").is_none());
    }

    #[test]
    fn setup_key_reissues_after_crash_use_and_duplicate_names() {
        let spec = EnsureSetupKeyInput {
            name: "edge".into(),
            key_type: "one-off".into(),
            expires_in: 3_600,
            auto_groups: vec!["group-id".into()],
            auto_group_names: None,
            usage_limit: 1,
            ephemeral: None,
            allow_extra_dns_labels: None,
            mutable_vault_file: "netbird-generated.vault".into(),
            mutable_vault_field: "edge_setup_key".into(),
        };
        let valid = api::SetupKey {
            id: Some("key-1".into()),
            name: Some("edge".into()),
            key_type: Some("one-off".into()),
            valid: Some(true),
            revoked: Some(false),
            usage_limit: Some(1),
            auto_groups: Some(vec!["group-id".into()]),
            ..Default::default()
        };
        assert!(!setup_key_reissue_required(&[valid.clone()], true, &spec));
        assert!(setup_key_reissue_required(&[valid.clone()], false, &spec));

        let used = api::SetupKey {
            valid: Some(false),
            used_times: Some(1),
            ..valid.clone()
        };
        assert!(setup_key_reissue_required(&[used], true, &spec));
        assert!(setup_key_reissue_required(
            &[valid.clone(), valid],
            true,
            &spec
        ));
    }
}
