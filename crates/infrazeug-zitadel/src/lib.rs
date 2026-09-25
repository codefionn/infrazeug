//! ZITADEL resource nodes for infrazeug playbooks.
//!
//! Projects, OIDC apps, project roles, and human users are reconciled against
//! ZITADEL's Management v1 API. Each resource uses its natural key for adoption.

use async_trait::async_trait;
use infrazeug_api::{builder::InfraBuilder, PlaybookBundle};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_ext_zitadel_api::{
    App, HumanEmail, HumanProfile, HumanUserInput, OidcAppInput, OidcConfigInput, ProjectInput,
    ProjectRoleInput, ProjectRoleUpdate,
};
use infrazeug_native::MethodRegistry;
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::OnceCell;

pub use infrazeug_ext_zitadel_api as api;
pub use infrazeug_ext_zitadel_api::ZitadelClient;
pub use infrazeug_resource::ResourceInput;

#[derive(Clone)]
pub enum ZitadelClientSource {
    Ready(Arc<ZitadelClient>),
    Vault {
        file: Arc<str>,
        cache: Arc<OnceCell<Arc<ZitadelClient>>>,
    },
}
impl ZitadelClientSource {
    pub fn ready(client: ZitadelClient) -> Self {
        Self::Ready(Arc::new(client))
    }
    /// Vault file with `base_url` and `token` fields.
    pub fn vault(file: impl Into<String>) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }
    async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<ZitadelClient>> {
        match self {
            Self::Ready(c) => Ok(c.clone()),
            Self::Vault { file, cache } => cache
                .get_or_try_init(|| async {
                    let base_url = ctx.read_secret_string(file, "base_url").await?;
                    let token = ctx.read_secret_string(file, "token").await?;
                    Ok(Arc::new(
                        ZitadelClient::new(base_url, token).map_err(ResourceError::provider)?,
                    ))
                })
                .await
                .cloned(),
        }
    }
}
pub fn client_from_env() -> anyhow::Result<ZitadelClient> {
    let url = std::env::var("ZITADEL_URL")?;
    let token = std::env::var("ZITADEL_TOKEN")?;
    Ok(ZitadelClient::new(url, token)?)
}

pub const ENSURE_PROJECT: &str = "zitadel.ensure_project";
pub const ENSURE_OIDC_APP: &str = "zitadel.ensure_oidc_app";
pub const ENSURE_PROJECT_ROLE: &str = "zitadel.ensure_project_role";
pub const ENSURE_HUMAN_USER: &str = "zitadel.ensure_human_user";

pub type EnsureProject = EnsureResource<ProjectResource>;
pub type EnsureOidcApp = EnsureResource<OidcAppResource>;
pub type EnsureProjectRole = EnsureResource<ProjectRoleResource>;
pub type EnsureHumanUser = EnsureResource<HumanUserResource>;
pub fn ensure_project(source: ZitadelClientSource) -> EnsureProject {
    EnsureResource::new(ProjectResource(source))
}
pub fn ensure_oidc_app(source: ZitadelClientSource) -> EnsureOidcApp {
    EnsureResource::new(OidcAppResource(source))
}
pub fn ensure_project_role(source: ZitadelClientSource) -> EnsureProjectRole {
    EnsureResource::new(ProjectRoleResource(source))
}
pub fn ensure_human_user(source: ZitadelClientSource) -> EnsureHumanUser {
    EnsureResource::new(HumanUserResource(source))
}
pub fn method_registry(source: ZitadelClientSource) -> MethodRegistry {
    let mut r = MethodRegistry::new();
    r.register(ensure_project(source.clone()));
    r.register(ensure_oidc_app(source.clone()));
    r.register(ensure_project_role(source.clone()));
    r.register(ensure_human_user(source));
    r
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureProjectInput {
    pub name: String,
    pub project_role_assertion: Option<bool>,
    pub project_role_check: Option<bool>,
    pub has_project_check: Option<bool>,
    pub private_labeling_setting: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureProjectOutput {
    pub id: String,
    pub name: String,
    pub project_role_assertion: bool,
    pub project_role_check: bool,
    pub has_project_check: bool,
    pub private_labeling_setting: Option<String>,
}
#[derive(Clone)]
pub struct ProjectResource(ZitadelClientSource);
fn project_state(p: api::Project) -> EnsureProjectOutput {
    EnsureProjectOutput {
        id: p.id,
        name: p.name,
        project_role_assertion: p.project_role_assertion,
        project_role_check: p.project_role_check,
        has_project_check: p.has_project_check,
        private_labeling_setting: p.private_labeling_setting,
    }
}
fn project_input(spec: &EnsureProjectInput, current: Option<&EnsureProjectOutput>) -> ProjectInput {
    ProjectInput {
        name: spec.name.clone(),
        project_role_assertion: spec
            .project_role_assertion
            .or(current.map(|c| c.project_role_assertion)),
        project_role_check: spec
            .project_role_check
            .or(current.map(|c| c.project_role_check)),
        has_project_check: spec
            .has_project_check
            .or(current.map(|c| c.has_project_check)),
        private_labeling_setting: spec
            .private_labeling_setting
            .clone()
            .or_else(|| current.and_then(|c| c.private_labeling_setting.clone())),
    }
}
#[async_trait]
impl Resource for ProjectResource {
    type Spec = EnsureProjectInput;
    type State = EnsureProjectOutput;
    fn kind(&self) -> &'static str {
        ENSURE_PROJECT
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        Ok(self
            .0
            .client(ctx)
            .await?
            .projects()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .find(|p| p.name == spec.name)
            .map(project_state))
    }
    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let c = self.0.client(ctx).await?;
        let id = c
            .create_project(&project_input(spec, None))
            .await
            .map_err(ResourceError::provider)?;
        Ok(project_state(
            c.project(&id).await.map_err(ResourceError::provider)?,
        ))
    }
    fn diff(&self, spec: &Self::Spec, c: &Self::State) -> Drift {
        let mut changes = Vec::new();
        if spec
            .project_role_assertion
            .is_some_and(|v| v != c.project_role_assertion)
        {
            changes.push("project_role_assertion");
        }
        if spec
            .project_role_check
            .is_some_and(|v| v != c.project_role_check)
        {
            changes.push("project_role_check");
        }
        if spec
            .has_project_check
            .is_some_and(|v| v != c.has_project_check)
        {
            changes.push("has_project_check");
        }
        if spec
            .private_labeling_setting
            .as_ref()
            .is_some_and(|v| Some(v) != c.private_labeling_setting.as_ref())
        {
            changes.push("private_labeling_setting");
        }
        drift(changes)
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let c = self.0.client(ctx).await?;
        c.update_project(&current.id, &project_input(spec, Some(&current)))
            .await
            .map_err(ResourceError::provider)?;
        Ok(project_state(
            c.project(&current.id)
                .await
                .map_err(ResourceError::provider)?,
        ))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureOidcAppInput {
    pub project_id: ResourceInput<String>,
    pub name: String,
    pub redirect_uris: Option<Vec<String>>,
    pub post_logout_redirect_uris: Option<Vec<String>>,
    pub app_type: Option<String>,
    pub auth_method_type: Option<String>,
    pub grant_types: Option<Vec<String>>,
    pub response_types: Option<Vec<String>>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureOidcAppOutput {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub post_logout_redirect_uris: Vec<String>,
    pub app_type: Option<String>,
    pub auth_method_type: Option<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    /// Returned only on create. Capture this value to a mutable vault immediately.
    pub client_secret: Option<String>,
}
#[derive(Clone)]
pub struct OidcAppResource(ZitadelClientSource);
fn oidc_state(app: App, client_secret: Option<String>) -> ResourceResult<EnsureOidcAppOutput> {
    let config = app.oidc_config.ok_or_else(|| {
        ResourceError::provider(anyhow::anyhow!("application {} is not an OIDC app", app.id))
    })?;
    Ok(EnsureOidcAppOutput {
        id: app.id,
        client_id: config.client_id,
        name: app.name,
        redirect_uris: config.redirect_uris,
        post_logout_redirect_uris: config.post_logout_redirect_uris,
        app_type: config.app_type,
        auth_method_type: config.auth_method_type,
        grant_types: config.grant_types,
        response_types: config.response_types,
        client_secret,
    })
}
fn config_input(s: &EnsureOidcAppInput, c: Option<&EnsureOidcAppOutput>) -> OidcConfigInput {
    OidcConfigInput {
        redirect_uris: s
            .redirect_uris
            .clone()
            .or_else(|| c.map(|v| v.redirect_uris.clone())),
        post_logout_redirect_uris: s
            .post_logout_redirect_uris
            .clone()
            .or_else(|| c.map(|v| v.post_logout_redirect_uris.clone())),
        app_type: s
            .app_type
            .clone()
            .or_else(|| c.and_then(|v| v.app_type.clone())),
        auth_method_type: s
            .auth_method_type
            .clone()
            .or_else(|| c.and_then(|v| v.auth_method_type.clone())),
        grant_types: s
            .grant_types
            .clone()
            .or_else(|| c.map(|v| v.grant_types.clone())),
        response_types: s
            .response_types
            .clone()
            .or_else(|| c.map(|v| v.response_types.clone())),
    }
}
fn list_eq(a: &[String], b: &[String]) -> bool {
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort();
    b.sort();
    a == b
}
#[async_trait]
impl Resource for OidcAppResource {
    type Spec = EnsureOidcAppInput;
    type State = EnsureOidcAppOutput;
    fn kind(&self) -> &'static str {
        ENSURE_OIDC_APP
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let c = self.0.client(ctx).await?;
        let project_id = s.project_id.resolve(ctx).await?;
        let app = c
            .apps(&project_id)
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .find(|a| a.name == s.name);
        match app {
            Some(a) => Ok(Some(oidc_state(
                c.app(&project_id, &a.id)
                    .await
                    .map_err(ResourceError::provider)?,
                None,
            )?)),
            None => Ok(None),
        }
    }
    async fn create(&self, ctx: &ResourceCtx, s: &Self::Spec) -> ResourceResult<Self::State> {
        let c = self.0.client(ctx).await?;
        let project_id = s.project_id.resolve(ctx).await?;
        let created = c
            .create_oidc_app(
                &project_id,
                &OidcAppInput {
                    name: s.name.clone(),
                    config: config_input(s, None),
                },
            )
            .await
            .map_err(ResourceError::provider)?;
        // The secret appears only in this response. Avoid a follow-up read that
        // could fail after creation and lose the value before vault capture.
        Ok(EnsureOidcAppOutput {
            id: created.app_id,
            client_id: created.client_id,
            name: s.name.clone(),
            redirect_uris: s.redirect_uris.clone().unwrap_or_default(),
            post_logout_redirect_uris: s.post_logout_redirect_uris.clone().unwrap_or_default(),
            app_type: s.app_type.clone(),
            auth_method_type: s.auth_method_type.clone(),
            grant_types: s.grant_types.clone().unwrap_or_default(),
            response_types: s.response_types.clone().unwrap_or_default(),
            client_secret: created.client_secret,
        })
    }
    fn diff(&self, s: &Self::Spec, c: &Self::State) -> Drift {
        let mut changes = Vec::new();
        if s.redirect_uris
            .as_ref()
            .is_some_and(|v| !list_eq(v, &c.redirect_uris))
        {
            changes.push("redirect_uris");
        }
        if s.post_logout_redirect_uris
            .as_ref()
            .is_some_and(|v| !list_eq(v, &c.post_logout_redirect_uris))
        {
            changes.push("post_logout_redirect_uris");
        }
        if s.app_type
            .as_ref()
            .is_some_and(|v| Some(v) != c.app_type.as_ref())
        {
            changes.push("app_type");
        }
        if s.auth_method_type
            .as_ref()
            .is_some_and(|v| Some(v) != c.auth_method_type.as_ref())
        {
            changes.push("auth_method_type");
        }
        if s.grant_types
            .as_ref()
            .is_some_and(|v| !list_eq(v, &c.grant_types))
        {
            changes.push("grant_types");
        }
        if s.response_types
            .as_ref()
            .is_some_and(|v| !list_eq(v, &c.response_types))
        {
            changes.push("response_types");
        }
        drift(changes)
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let c = self.0.client(ctx).await?;
        let project_id = s.project_id.resolve(ctx).await?;
        c.update_oidc_config(&project_id, &current.id, &config_input(s, Some(&current)))
            .await
            .map_err(ResourceError::provider)?;
        oidc_state(
            c.app(&project_id, &current.id)
                .await
                .map_err(ResourceError::provider)?,
            None,
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureProjectRoleInput {
    pub project_id: ResourceInput<String>,
    pub role_key: String,
    pub display_name: String,
    pub group: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureProjectRoleOutput {
    pub role_key: String,
    pub display_name: String,
    pub group: Option<String>,
}
#[derive(Clone)]
pub struct ProjectRoleResource(ZitadelClientSource);
fn role_state(r: api::ProjectRole) -> EnsureProjectRoleOutput {
    EnsureProjectRoleOutput {
        role_key: r.key,
        display_name: r.display_name,
        group: r.group,
    }
}
#[async_trait]
impl Resource for ProjectRoleResource {
    type Spec = EnsureProjectRoleInput;
    type State = EnsureProjectRoleOutput;
    fn kind(&self) -> &'static str {
        ENSURE_PROJECT_ROLE
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let project_id = s.project_id.resolve(ctx).await?;
        Ok(self
            .0
            .client(ctx)
            .await?
            .roles(&project_id)
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .find(|r| r.key == s.role_key)
            .map(role_state))
    }
    async fn create(&self, ctx: &ResourceCtx, s: &Self::Spec) -> ResourceResult<Self::State> {
        let c = self.0.client(ctx).await?;
        let project_id = s.project_id.resolve(ctx).await?;
        c.create_role(
            &project_id,
            &ProjectRoleInput {
                role_key: s.role_key.clone(),
                display_name: s.display_name.clone(),
                group: s.group.clone(),
            },
        )
        .await
        .map_err(ResourceError::provider)?;
        self.observe(ctx, s)
            .await?
            .ok_or_else(|| ResourceError::provider(anyhow::anyhow!("created role not visible")))
    }
    fn diff(&self, s: &Self::Spec, c: &Self::State) -> Drift {
        let mut changes = Vec::new();
        if s.display_name != c.display_name {
            changes.push("display_name");
        }
        if s.group
            .as_ref()
            .is_some_and(|g| Some(g) != c.group.as_ref())
        {
            changes.push("group");
        }
        drift(changes)
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
        c: Self::State,
    ) -> ResourceResult<Self::State> {
        let project_id = s.project_id.resolve(ctx).await?;
        self.0
            .client(ctx)
            .await?
            .update_role(
                &project_id,
                &s.role_key,
                &ProjectRoleUpdate {
                    display_name: s.display_name.clone(),
                    group: s.group.clone().or(c.group),
                },
            )
            .await
            .map_err(ResourceError::provider)?;
        self.observe(ctx, s)
            .await?
            .ok_or_else(|| ResourceError::provider(anyhow::anyhow!("updated role not visible")))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureHumanUserInput {
    pub user_name: String,
    pub first_name: String,
    pub last_name: String,
    pub display_name: String,
    pub email: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnsureHumanUserOutput {
    pub id: String,
    pub user_name: String,
    pub first_name: String,
    pub last_name: String,
    pub display_name: String,
    pub email: String,
}
#[derive(Clone)]
pub struct HumanUserResource(ZitadelClientSource);
fn user_state(u: api::User) -> ResourceResult<EnsureHumanUserOutput> {
    let human = u
        .human
        .ok_or_else(|| ResourceError::provider(anyhow::anyhow!("user {} is not human", u.id)))?;
    Ok(EnsureHumanUserOutput {
        id: u.id,
        user_name: u.user_name,
        first_name: human.profile.first_name,
        last_name: human.profile.last_name,
        display_name: human.profile.display_name,
        email: human.email.email,
    })
}
#[async_trait]
impl Resource for HumanUserResource {
    type Spec = EnsureHumanUserInput;
    type State = EnsureHumanUserOutput;
    fn kind(&self) -> &'static str {
        ENSURE_HUMAN_USER
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        self.0
            .client(ctx)
            .await?
            .users()
            .await
            .map_err(ResourceError::provider)?
            .into_iter()
            .find(|u| u.user_name == s.user_name)
            .map(user_state)
            .transpose()
    }
    async fn create(&self, ctx: &ResourceCtx, s: &Self::Spec) -> ResourceResult<Self::State> {
        let c = self.0.client(ctx).await?;
        let id = c
            .create_human_user(&HumanUserInput {
                user_name: s.user_name.clone(),
                profile: HumanProfile {
                    first_name: s.first_name.clone(),
                    last_name: s.last_name.clone(),
                    display_name: s.display_name.clone(),
                },
                email: HumanEmail {
                    email: s.email.clone(),
                    is_email_verified: false,
                },
            })
            .await
            .map_err(ResourceError::provider)?;
        user_state(c.user(&id).await.map_err(ResourceError::provider)?)
    }
    fn diff(&self, s: &Self::Spec, c: &Self::State) -> Drift {
        let mut changes = Vec::new();
        if s.first_name != c.first_name {
            changes.push("first_name");
        }
        if s.last_name != c.last_name {
            changes.push("last_name");
        }
        if s.display_name != c.display_name {
            changes.push("display_name");
        }
        if s.email != c.email {
            changes.push("email");
        }
        drift(changes)
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        s: &Self::Spec,
        c: Self::State,
    ) -> ResourceResult<Self::State> {
        let client = self.0.client(ctx).await?;
        if s.first_name != c.first_name
            || s.last_name != c.last_name
            || s.display_name != c.display_name
        {
            client
                .update_human_profile(
                    &c.id,
                    &HumanProfile {
                        first_name: s.first_name.clone(),
                        last_name: s.last_name.clone(),
                        display_name: s.display_name.clone(),
                    },
                )
                .await
                .map_err(ResourceError::provider)?;
        }
        if s.email != c.email {
            client
                .update_human_email(&c.id, &s.email)
                .await
                .map_err(ResourceError::provider)?;
        }
        user_state(client.user(&c.id).await.map_err(ResourceError::provider)?)
    }
}
fn drift(changes: Vec<&str>) -> Drift {
    if changes.is_empty() {
        Drift::InSync
    } else {
        Drift::Drifted(changes.join(", "))
    }
}

pub trait ZitadelInfraExt {
    fn zitadel(self, client: ZitadelClient, machine_id: MachineId) -> ZitadelInfraBuilder;
    fn zitadel_vault(self, file: impl Into<String>, machine_id: MachineId) -> ZitadelInfraBuilder;
}
impl ZitadelInfraExt for InfraBuilder {
    fn zitadel(self, client: ZitadelClient, machine_id: MachineId) -> ZitadelInfraBuilder {
        ZitadelInfraBuilder::new(self, ZitadelClientSource::ready(client), machine_id)
    }
    fn zitadel_vault(self, file: impl Into<String>, machine_id: MachineId) -> ZitadelInfraBuilder {
        ZitadelInfraBuilder::new(self, ZitadelClientSource::vault(file), machine_id)
    }
}
pub struct ZitadelInfraBuilder {
    builder: InfraBuilder,
    machine_id: MachineId,
}
impl ZitadelInfraBuilder {
    pub fn new(builder: InfraBuilder, source: ZitadelClientSource, machine_id: MachineId) -> Self {
        let builder = builder
            .method(ensure_project(source.clone()))
            .method(ensure_oidc_app(source.clone()))
            .method(ensure_project_role(source.clone()))
            .method(ensure_human_user(source));
        Self {
            builder,
            machine_id,
        }
    }
    pub fn ensure_project(
        self,
        id: NodeId,
        name: &str,
        input: EnsureProjectInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureProject>(id, name, self.machine_id, input)?
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }
    pub fn ensure_oidc_app(
        self,
        id: NodeId,
        name: &str,
        input: EnsureOidcAppInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureOidcApp>(id, name, self.machine_id, input)?
            .deps(deps)
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }
    pub fn ensure_project_role(
        self,
        id: NodeId,
        name: &str,
        input: EnsureProjectRoleInput,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureProjectRole>(id, name, self.machine_id, input)?
            .deps(deps)
            .always()
            .build()?;
        Ok(Self {
            builder,
            machine_id: self.machine_id,
        })
    }
    pub fn ensure_human_user(
        self,
        id: NodeId,
        name: &str,
        input: EnsureHumanUserInput,
    ) -> anyhow::Result<Self> {
        let builder = self
            .builder
            .native_typed::<EnsureHumanUser>(id, name, self.machine_id, input)?
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
    use infrazeug_native::{NodeCtx, NodeMethod, PlanCtx, PlanMethodOutcome, SecretSource};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use uuid::Uuid;

    struct ProjectCapture {
        node: Uuid,
        machine: Uuid,
    }
    #[async_trait]
    impl SecretSource for ProjectCapture {
        async fn read_field(&self, _: &str, _: &str) -> infrazeug_native::Result<Vec<u8>> {
            unreachable!()
        }
        fn has_node_captures(&self) -> bool {
            true
        }
        async fn read_node_capture(
            &self,
            node: Uuid,
            machine: Uuid,
        ) -> infrazeug_native::Result<Vec<u8>> {
            assert_eq!((node, machine), (self.node, self.machine));
            Ok(br#"{"id":"project-42"}"#.to_vec())
        }
    }

    #[tokio::test]
    async fn project_capture_drives_role_lifecycle() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in [
                r#"{"result":[]}"#,
                r#"{"details":{}}"#,
                r#"{"details":{"totalResult":1},"result":[{"key":"admin","displayName":"Administrator","group":"staff"}]}"#,
                r#"{"details":{"totalResult":1},"result":[{"key":"admin","displayName":"Administrator","group":"staff"}]}"#,
            ] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0u8; 1024];
                    let n = socket.read(&mut chunk).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(headers_end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers_end = headers_end + 4;
                        let headers = String::from_utf8_lossy(&bytes[..headers_end]);
                        let length = headers
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|v| v.parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= headers_end + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8_lossy(&bytes).into_owned());
                let reply = format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", response.len(), response);
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
            requests
        });
        let machine = Uuid::new_v4();
        let project_node = Uuid::new_v4();
        let source: Arc<dyn SecretSource> = Arc::new(ProjectCapture {
            node: project_node,
            machine,
        });
        let ctx =
            ResourceCtx::from(&NodeCtx::new(machine, Uuid::new_v4()).with_secrets(Some(source)));
        let client = ZitadelClient::new(format!("http://{address}"), "secret-token").unwrap();
        let resource = ProjectRoleResource(ZitadelClientSource::ready(client));
        let spec = EnsureProjectRoleInput {
            project_id: ResourceInput::node(project_node).json_pointer("/id"),
            role_key: "admin".into(),
            display_name: "Administrator".into(),
            group: Some("staff".into()),
        };
        assert!(resource.observe(&ctx, &spec).await.unwrap().is_none());
        let created = resource.create(&ctx, &spec).await.unwrap();
        assert_eq!(created.role_key, "admin");
        assert_eq!(resource.diff(&spec, &created), Drift::InSync);
        let observed = resource.observe(&ctx, &spec).await.unwrap().unwrap();
        assert_eq!(resource.diff(&spec, &observed), Drift::InSync);
        let requests = server.await.unwrap();
        assert!(requests
            .iter()
            .all(|r| r.contains("/projects/project-42/roles")));
        assert!(requests
            .iter()
            .all(|r| r.contains("authorization: Bearer secret-token")
                || r.contains("Authorization: Bearer secret-token")));
        assert!(requests[1].contains("\"roleKey\":\"admin\""));
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.starts_with("POST /management/v1/projects/project-42/roles HTTP"))
                .count(),
            1
        );
    }

    #[test]
    fn only_declared_project_fields_drift() {
        let resource = ProjectResource(ZitadelClientSource::vault("id.vault"));
        let current = EnsureProjectOutput {
            id: "p".into(),
            name: "name".into(),
            project_role_assertion: true,
            project_role_check: false,
            has_project_check: true,
            private_labeling_setting: Some("PRIVATE_LABELING_SETTING_UNSPECIFIED".into()),
        };
        let spec = EnsureProjectInput {
            name: "name".into(),
            ..Default::default()
        };
        assert_eq!(resource.diff(&spec, &current), Drift::InSync);
        let changed = EnsureProjectInput {
            project_role_check: Some(true),
            ..spec
        };
        assert!(matches!(
            resource.diff(&changed, &current),
            Drift::Drifted(_)
        ));
    }

    #[tokio::test]
    async fn vault_preview_is_unknown_without_secret_source() {
        let method = ensure_project(ZitadelClientSource::vault("identity.vault"));
        let ctx = PlanCtx::new(Uuid::nil(), Uuid::nil());
        let result = method
            .plan(
                &ctx,
                &EnsureProjectInput {
                    name: "identity".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(result, PlanMethodOutcome::Unknown);
    }

    #[test]
    fn builder_registers_project_and_dependent_role() {
        let machine = MachineId(Uuid::new_v4());
        let project = NodeId(Uuid::new_v4());
        let role = NodeId(Uuid::new_v4());
        let client = ZitadelClient::new("https://example.zitadel.cloud", "pat").unwrap();
        let bundle = InfraBuilder::new()
            .machine(builder::controller(machine))
            .unwrap()
            .zitadel(client, machine)
            .ensure_project(
                project,
                "project",
                EnsureProjectInput {
                    name: "identity".into(),
                    ..Default::default()
                },
            )
            .unwrap()
            .ensure_project_role(
                role,
                "admin",
                EnsureProjectRoleInput {
                    project_id: ResourceInput::node(project.0).json_pointer("/id"),
                    role_key: "admin".into(),
                    display_name: "Administrator".into(),
                    group: None,
                },
                [project],
            )
            .unwrap()
            .finish();
        bundle.plan().unwrap();
    }
}
