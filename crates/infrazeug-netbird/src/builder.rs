//! Fluent builder extension for NetBird resource nodes.

use crate::{
    ensure_dns_settings, ensure_group, ensure_identity_provider, ensure_nameserver_group,
    ensure_network, ensure_network_resource, ensure_network_router, ensure_policy,
    ensure_reverse_proxy_domain, ensure_reverse_proxy_service, ensure_reverse_proxy_token,
    ensure_route, ensure_setup_key, EnsureDnsSettings, EnsureDnsSettingsInput, EnsureGroup,
    EnsureGroupInput, EnsureIdentityProvider, EnsureIdentityProviderInput, EnsureNameserverGroup,
    EnsureNameserverGroupInput, EnsureNetwork, EnsureNetworkInput, EnsureNetworkResource,
    EnsureNetworkResourceInput, EnsureNetworkRouter, EnsureNetworkRouterInput, EnsurePolicy,
    EnsurePolicyInput, EnsureReverseProxyDomain, EnsureReverseProxyDomainInput,
    EnsureReverseProxyService, EnsureReverseProxyServiceInput, EnsureReverseProxyToken,
    EnsureReverseProxyTokenInput, EnsureRoute, EnsureRouteInput, EnsureSetupKey,
    EnsureSetupKeyInput, NetBirdClientSource,
};
use infrazeug_api::{builder::InfraBuilder, PlaybookBundle};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_ext_netbird_api::NetBirdClient;

/// Attach NetBird's controller-side native methods to an infra builder.
pub trait NetBirdInfraExt {
    fn netbird(self, client: NetBirdClient, machine_id: MachineId) -> NetBirdInfraBuilder;
    fn netbird_vault(self, file: impl Into<String>, machine_id: MachineId) -> NetBirdInfraBuilder;
    /// Read the Management API credential from `files/mutable/{file}`.
    fn netbird_mutable_vault(
        self,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> NetBirdInfraBuilder;
    fn netbird_source(
        self,
        source: NetBirdClientSource,
        machine_id: MachineId,
    ) -> NetBirdInfraBuilder;
}
impl NetBirdInfraExt for InfraBuilder {
    fn netbird(self, client: NetBirdClient, machine_id: MachineId) -> NetBirdInfraBuilder {
        NetBirdInfraBuilder::new(self, NetBirdClientSource::ready(client), machine_id)
    }
    fn netbird_vault(self, file: impl Into<String>, machine_id: MachineId) -> NetBirdInfraBuilder {
        NetBirdInfraBuilder::new(self, NetBirdClientSource::vault(file), machine_id)
    }
    fn netbird_mutable_vault(
        self,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> NetBirdInfraBuilder {
        NetBirdInfraBuilder::new(self, NetBirdClientSource::mutable_vault(file), machine_id)
    }
    fn netbird_source(
        self,
        source: NetBirdClientSource,
        machine_id: MachineId,
    ) -> NetBirdInfraBuilder {
        NetBirdInfraBuilder::new(self, source, machine_id)
    }
}

/// Staged builder with NetBird methods registered on its local controller.
pub struct NetBirdInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}
impl NetBirdInfraBuilder {
    pub fn new(builder: InfraBuilder, source: NetBirdClientSource, machine_id: MachineId) -> Self {
        let builder = builder
            .method(ensure_group(source.clone()))
            .method(ensure_identity_provider(source.clone()))
            .method(ensure_policy(source.clone()))
            .method(ensure_route(source.clone()))
            .method(ensure_network(source.clone()))
            .method(ensure_network_resource(source.clone()))
            .method(ensure_network_router(source.clone()))
            .method(ensure_nameserver_group(source.clone()))
            .method(ensure_reverse_proxy_domain(source.clone()))
            .method(ensure_reverse_proxy_service(source.clone()))
            .method(ensure_setup_key(source.clone()))
            .method(ensure_reverse_proxy_token(source.clone()))
            .method(ensure_dns_settings(source));
        Self {
            builder,
            machine_id,
        }
    }
    pub fn ensure_group(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureGroupInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsureGroup>(node_id, name, input)
    }
    pub fn ensure_group_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureGroupInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsureGroup>(node_id, name, input, deps)
    }
    pub fn ensure_identity_provider(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureIdentityProviderInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsureIdentityProvider>(node_id, name, input)
    }
    pub fn ensure_identity_provider_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureIdentityProviderInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsureIdentityProvider>(node_id, name, input, deps)
    }
    pub fn ensure_policy(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsurePolicyInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsurePolicy>(node_id, name, input)
    }
    pub fn ensure_policy_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsurePolicyInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsurePolicy>(node_id, name, input, deps)
    }
    pub fn ensure_route(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureRouteInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsureRoute>(node_id, name, input)
    }
    pub fn ensure_route_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureRouteInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsureRoute>(node_id, name, input, deps)
    }
    pub fn ensure_network(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureNetworkInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsureNetwork>(node_id, name, input)
    }
    pub fn ensure_network_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureNetworkInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsureNetwork>(node_id, name, input, deps)
    }
    pub fn ensure_network_resource(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureNetworkResourceInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsureNetworkResource>(node_id, name, input)
    }
    /// Ensure a network resource after its network and referenced groups exist.
    pub fn ensure_network_resource_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureNetworkResourceInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsureNetworkResource>(node_id, name, input, deps)
    }
    pub fn ensure_network_router(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureNetworkRouterInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsureNetworkRouter>(node_id, name, input)
    }
    /// Ensure a network router after its network and referenced groups exist.
    pub fn ensure_network_router_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureNetworkRouterInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsureNetworkRouter>(node_id, name, input, deps)
    }
    pub fn ensure_nameserver_group(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureNameserverGroupInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsureNameserverGroup>(node_id, name, input)
    }
    pub fn ensure_nameserver_group_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureNameserverGroupInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsureNameserverGroup>(node_id, name, input, deps)
    }
    pub fn ensure_dns_settings(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureDnsSettingsInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsureDnsSettings>(node_id, name, input)
    }
    pub fn ensure_dns_settings_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureDnsSettingsInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsureDnsSettings>(node_id, name, input, deps)
    }
    pub fn ensure_reverse_proxy_domain(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureReverseProxyDomainInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsureReverseProxyDomain>(node_id, name, input)
    }
    pub fn ensure_reverse_proxy_domain_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureReverseProxyDomainInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsureReverseProxyDomain>(node_id, name, input, deps)
    }
    pub fn ensure_reverse_proxy_service(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureReverseProxyServiceInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsureReverseProxyService>(node_id, name, input)
    }
    pub fn ensure_reverse_proxy_service_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureReverseProxyServiceInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsureReverseProxyService>(node_id, name, input, deps)
    }
    pub fn ensure_setup_key(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureSetupKeyInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsureSetupKey>(node_id, name, input)
    }
    pub fn ensure_setup_key_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureSetupKeyInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsureSetupKey>(node_id, name, input, deps)
    }
    pub fn ensure_reverse_proxy_token(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureReverseProxyTokenInput,
    ) -> anyhow::Result<Self> {
        self.add::<EnsureReverseProxyToken>(node_id, name, input)
    }
    pub fn ensure_reverse_proxy_token_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureReverseProxyTokenInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.add_after::<EnsureReverseProxyToken>(node_id, name, input, deps)
    }
    fn add<M>(self, node_id: NodeId, name: &str, input: M::Input) -> anyhow::Result<Self>
    where
        M: infrazeug_native::NodeMethod + 'static,
    {
        let builder = self
            .builder
            .native_typed::<M>(node_id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }
    fn add_after<M>(
        self,
        node_id: NodeId,
        name: &str,
        input: M::Input,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self>
    where
        M: infrazeug_native::NodeMethod + 'static,
    {
        let builder = self
            .builder
            .native_typed::<M>(node_id, name, self.machine_id, input)?
            .deps(deps)
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }
    pub fn into_builder(self) -> InfraBuilder {
        self.builder
    }
    pub fn finish(self) -> PlaybookBundle {
        self.builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_api::builder;
    use infrazeug_ext_netbird_api::{Auth, NetBirdConfig};
    use uuid::Uuid;
    #[test]
    fn group_node_wires_into_a_plan() {
        let machine = MachineId(Uuid::new_v4());
        let node = NodeId(Uuid::new_v4());
        let client = NetBirdClient::new(NetBirdConfig::new(Auth::personal_access_token("test")));
        let bundle = InfraBuilder::new()
            .machine(builder::controller(machine))
            .unwrap()
            .netbird(client, machine)
            .ensure_group(
                node,
                "ops",
                EnsureGroupInput {
                    name: "ops".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .finish();
        bundle.plan().expect("wiring must plan");
    }
}
