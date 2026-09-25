//! Authentik native ensure nodes for infrazeug playbooks.
//! Methods are idempotent, preserve fields outside the input, and capture server IDs.

mod client;
mod methods;

pub use client::{client_from_env, AuthentikClientSource};
pub use infrazeug_ext_authentik_api::{AuthentikClient, RedirectUri};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
pub use methods::{
    ensure_application, ensure_group, ensure_oauth2_provider, ensure_user, ApplicationResource,
    EnsureApplication, EnsureApplicationInput, EnsureApplicationOutput, EnsureGroup,
    EnsureGroupInput, EnsureGroupOutput, EnsureOAuth2Provider, EnsureOAuth2ProviderInput,
    EnsureOAuth2ProviderOutput, EnsureUser, EnsureUserInput, EnsureUserOutput, GroupResource,
    OAuth2ProviderResource, UserResource, ENSURE_APPLICATION, ENSURE_GROUP, ENSURE_OAUTH2_PROVIDER,
    ENSURE_USER,
};

use infrazeug_api::{builder::InfraBuilder, PlaybookBundle};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_native::MethodRegistry;

/// Register all Authentik methods for an agent or playbook runner.
pub fn method_registry(source: AuthentikClientSource) -> MethodRegistry {
    let mut registry = MethodRegistry::new();
    registry.register(ensure_user(source.clone()));
    registry.register(ensure_group(source.clone()));
    registry.register(ensure_application(source.clone()));
    registry.register(ensure_oauth2_provider(source));
    registry
}

/// Attach Authentik native methods to an infra builder.
pub trait AuthentikInfraExt {
    fn authentik(self, client: AuthentikClient, machine_id: MachineId) -> AuthentikInfraBuilder;
    fn authentik_vault(
        self,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> AuthentikInfraBuilder;
}
impl AuthentikInfraExt for InfraBuilder {
    fn authentik(self, client: AuthentikClient, machine_id: MachineId) -> AuthentikInfraBuilder {
        AuthentikInfraBuilder::new(self, AuthentikClientSource::ready(client), machine_id)
    }
    fn authentik_vault(
        self,
        file: impl Into<String>,
        machine_id: MachineId,
    ) -> AuthentikInfraBuilder {
        AuthentikInfraBuilder::new(self, AuthentikClientSource::vault(file), machine_id)
    }
}

/// Staged builder with Authentik methods registered.
pub struct AuthentikInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}
impl AuthentikInfraBuilder {
    pub fn new(
        builder: InfraBuilder,
        source: AuthentikClientSource,
        machine_id: MachineId,
    ) -> Self {
        let builder = builder
            .method(ensure_user(source.clone()))
            .method(ensure_group(source.clone()))
            .method(ensure_application(source.clone()))
            .method(ensure_oauth2_provider(source));
        Self {
            builder,
            machine_id,
        }
    }
    pub fn ensure_user(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureUserInput,
    ) -> anyhow::Result<Self> {
        self.node::<EnsureUser>(node_id, name, input, [])
    }
    pub fn ensure_group(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureGroupInput,
    ) -> anyhow::Result<Self> {
        self.node::<EnsureGroup>(node_id, name, input, [])
    }
    pub fn ensure_application(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureApplicationInput,
    ) -> anyhow::Result<Self> {
        self.node::<EnsureApplication>(node_id, name, input, [])
    }
    pub fn ensure_application_after(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureApplicationInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        self.node::<EnsureApplication>(node_id, name, input, deps)
    }
    pub fn ensure_oauth2_provider(
        self,
        node_id: NodeId,
        name: &str,
        input: EnsureOAuth2ProviderInput,
    ) -> anyhow::Result<Self> {
        self.node::<EnsureOAuth2Provider>(node_id, name, input, [])
    }
    fn node<M: infrazeug_native::NodeMethod + 'static>(
        self,
        node_id: NodeId,
        name: &str,
        input: M::Input,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
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
    use infrazeug_native::PlanCtx;
    use uuid::Uuid;

    #[tokio::test]
    async fn vault_source_without_unlocked_vault_reports_unknown() {
        let source = AuthentikClientSource::vault("auth/authentik.vault");
        let ctx = ResourceCtx::from(&PlanCtx::new(Uuid::nil(), Uuid::nil()));
        assert!(matches!(
            source.client(&ctx).await,
            Err(ResourceError::SecretsUnavailable)
        ));
    }

    #[test]
    fn builder_registers_authentik_nodes() {
        let machine = MachineId(Uuid::new_v4());
        let user = NodeId(Uuid::new_v4());
        let bundle = InfraBuilder::new()
            .machine(builder::controller(machine))
            .unwrap()
            .authentik_vault("auth/authentik.vault", machine)
            .ensure_user(
                user,
                "alice",
                EnsureUserInput {
                    username: "alice".into(),
                    name: "Alice".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .finish();
        let authored_nodes = bundle
            .infra
            .nodes
            .iter()
            .filter(|node| !(node.body.is_group_bookend() || node.body.is_connect()))
            .count();
        assert_eq!(authored_nodes, 1);
        bundle.plan().unwrap();
    }
}
