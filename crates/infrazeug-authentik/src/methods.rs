use crate::client::AuthentikClientSource;
use async_trait::async_trait;
use infrazeug_ext_authentik_api::{
    Application, ApplicationWrite, AuthentikClient, Group, GroupWrite, OAuth2Provider,
    OAuth2ProviderWrite, RedirectUri, User, UserWrite,
};
use infrazeug_resource::{
    Drift, EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const ENSURE_USER: &str = "authentik.ensure_user";
pub const ENSURE_GROUP: &str = "authentik.ensure_group";
pub const ENSURE_APPLICATION: &str = "authentik.ensure_application";
pub const ENSURE_OAUTH2_PROVIDER: &str = "authentik.ensure_oauth2_provider";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureUserInput {
    pub username: String,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub groups: Option<Vec<String>>,
}
pub type EnsureUserOutput = User;
#[derive(Clone)]
pub struct UserResource {
    source: AuthentikClientSource,
}
impl UserResource {
    pub fn new(source: AuthentikClientSource) -> Self {
        Self { source }
    }
}
pub type EnsureUser = EnsureResource<UserResource>;
pub fn ensure_user(source: AuthentikClientSource) -> EnsureUser {
    EnsureResource::new(UserResource::new(source))
}

#[async_trait]
impl Resource for UserResource {
    type Spec = EnsureUserInput;
    type State = EnsureUserOutput;
    fn kind(&self) -> &'static str {
        ENSURE_USER
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let found = self
            .source
            .client(ctx)
            .await?
            .users(&spec.username)
            .await
            .map_err(ResourceError::provider)?;
        single_exact(
            found,
            |u| u.username == spec.username,
            "user",
            &spec.username,
        )
    }
    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let body = UserWrite {
            username: spec.username.clone(),
            name: spec.name.clone(),
            email: spec.email.clone(),
            is_active: spec.is_active.or(Some(true)),
            path: spec.path.clone(),
            groups: spec.groups.clone(),
        };
        self.source
            .client(ctx)
            .await?
            .create_user(&body)
            .await
            .map_err(ResourceError::provider)
    }
    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut changed = Vec::new();
        if spec.name != current.name {
            changed.push("name");
        }
        if spec.email.as_ref().is_some_and(|v| v != &current.email) {
            changed.push("email");
        }
        if spec.is_active.is_some_and(|v| v != current.is_active) {
            changed.push("is_active");
        }
        if spec.path.as_ref().is_some_and(|v| v != &current.path) {
            changed.push("path");
        }
        if spec
            .groups
            .as_ref()
            .is_some_and(|v| !same_set(v, &current.groups))
        {
            changed.push("groups");
        }
        drift(changed)
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let mut patch = Map::new();
        patch.insert("name".into(), json!(spec.name));
        insert_option(&mut patch, "email", &spec.email);
        insert_option(&mut patch, "is_active", &spec.is_active);
        insert_option(&mut patch, "path", &spec.path);
        insert_option(&mut patch, "groups", &spec.groups);
        self.source
            .client(ctx)
            .await?
            .patch_user(current.pk, &Value::Object(patch))
            .await
            .map_err(ResourceError::provider)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureGroupInput {
    pub name: String,
    #[serde(default)]
    pub is_superuser: Option<bool>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub users: Option<Vec<i64>>,
}
pub type EnsureGroupOutput = Group;
#[derive(Clone)]
pub struct GroupResource {
    source: AuthentikClientSource,
}
impl GroupResource {
    pub fn new(source: AuthentikClientSource) -> Self {
        Self { source }
    }
}
pub type EnsureGroup = EnsureResource<GroupResource>;
pub fn ensure_group(source: AuthentikClientSource) -> EnsureGroup {
    EnsureResource::new(GroupResource::new(source))
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
        let found = self
            .source
            .client(ctx)
            .await?
            .groups(&spec.name)
            .await
            .map_err(ResourceError::provider)?;
        single_exact(found, |g| g.name == spec.name, "group", &spec.name)
    }
    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let body = GroupWrite {
            name: spec.name.clone(),
            is_superuser: spec.is_superuser,
            parent: spec.parent.clone(),
            users: spec.users.clone(),
        };
        self.source
            .client(ctx)
            .await?
            .create_group(&body)
            .await
            .map_err(ResourceError::provider)
    }
    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut changed = Vec::new();
        if spec.is_superuser.is_some_and(|v| v != current.is_superuser) {
            changed.push("is_superuser");
        }
        if spec
            .parent
            .as_ref()
            .is_some_and(|v| Some(v) != current.parent.as_ref())
        {
            changed.push("parent");
        }
        if spec
            .users
            .as_ref()
            .is_some_and(|v| !same_set(v, &current.users))
        {
            changed.push("users");
        }
        drift(changed)
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let mut patch = Map::new();
        insert_option(&mut patch, "is_superuser", &spec.is_superuser);
        insert_option(&mut patch, "parent", &spec.parent);
        insert_option(&mut patch, "users", &spec.users);
        self.source
            .client(ctx)
            .await?
            .patch_group(&current.pk, &Value::Object(patch))
            .await
            .map_err(ResourceError::provider)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureApplicationInput {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub provider: Option<i64>,
    /// Resolve an OAuth2 provider by exact name during apply. Useful after an
    /// `ensure_oauth2_provider` node created it in the same run.
    #[serde(default)]
    pub provider_name: Option<String>,
    #[serde(default)]
    pub meta_description: Option<String>,
    #[serde(default)]
    pub meta_publisher: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnsureApplicationOutput {
    pub pk: String,
    pub slug: String,
    pub name: String,
    pub provider: Option<i64>,
    pub provider_name: Option<String>,
    pub meta_description: String,
    pub meta_publisher: String,
}
#[derive(Clone)]
pub struct ApplicationResource {
    source: AuthentikClientSource,
}
impl ApplicationResource {
    pub fn new(source: AuthentikClientSource) -> Self {
        Self { source }
    }
}
pub type EnsureApplication = EnsureResource<ApplicationResource>;
pub fn ensure_application(source: AuthentikClientSource) -> EnsureApplication {
    EnsureResource::new(ApplicationResource::new(source))
}

#[async_trait]
impl Resource for ApplicationResource {
    type Spec = EnsureApplicationInput;
    type State = EnsureApplicationOutput;
    fn kind(&self) -> &'static str {
        ENSURE_APPLICATION
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let app = client
            .application(&spec.slug)
            .await
            .map_err(ResourceError::provider)?;
        match app {
            Some(app) => Ok(Some(
                application_state(&client, app, spec.provider_name.is_some()).await?,
            )),
            None => Ok(None),
        }
    }
    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let client = self.source.client(ctx).await?;
        let provider = application_provider_id(&client, spec).await?;
        let body = ApplicationWrite {
            slug: spec.slug.clone(),
            name: spec.name.clone(),
            provider,
            meta_description: spec.meta_description.clone(),
            meta_publisher: spec.meta_publisher.clone(),
        };
        let app = client
            .create_application(&body)
            .await
            .map_err(ResourceError::provider)?;
        application_state(&client, app, spec.provider_name.is_some()).await
    }
    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut changed = Vec::new();
        if spec.name != current.name {
            changed.push("name");
        }
        if spec.provider_name.is_none()
            && spec.provider.is_some_and(|v| Some(v) != current.provider)
        {
            changed.push("provider");
        }
        if spec
            .provider_name
            .as_ref()
            .is_some_and(|v| Some(v) != current.provider_name.as_ref())
        {
            changed.push("provider_name");
        }
        if spec
            .meta_description
            .as_ref()
            .is_some_and(|v| v != &current.meta_description)
        {
            changed.push("meta_description");
        }
        if spec
            .meta_publisher
            .as_ref()
            .is_some_and(|v| v != &current.meta_publisher)
        {
            changed.push("meta_publisher");
        }
        drift(changed)
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let mut patch = Map::new();
        patch.insert("name".into(), json!(spec.name));
        let client = self.source.client(ctx).await?;
        let provider = application_provider_id(&client, spec).await?;
        insert_option(&mut patch, "provider", &provider);
        insert_option(&mut patch, "meta_description", &spec.meta_description);
        insert_option(&mut patch, "meta_publisher", &spec.meta_publisher);
        let app = client
            .patch_application(&current.slug, &Value::Object(patch))
            .await
            .map_err(ResourceError::provider)?;
        application_state(&client, app, spec.provider_name.is_some()).await
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EnsureOAuth2ProviderInput {
    pub name: String,
    pub authorization_flow: String,
    pub invalidation_flow: String,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_type: Option<String>,
    #[serde(default)]
    pub redirect_uris: Option<Vec<RedirectUri>>,
}
/// The `client_secret` is captured for vault writes. Avoid logging captures.
#[derive(Clone, Serialize, Deserialize)]
pub struct EnsureOAuth2ProviderOutput {
    pub pk: i64,
    pub name: String,
    pub authorization_flow: String,
    pub invalidation_flow: String,
    pub client_id: String,
    pub client_type: String,
    pub redirect_uris: Vec<RedirectUri>,
    pub client_secret: Option<String>,
}
impl std::fmt::Debug for EnsureOAuth2ProviderOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnsureOAuth2ProviderOutput")
            .field("pk", &self.pk)
            .field("name", &self.name)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[redacted]")
            .finish()
    }
}
impl From<OAuth2Provider> for EnsureOAuth2ProviderOutput {
    fn from(p: OAuth2Provider) -> Self {
        let secret = if p.client_secret.is_empty() {
            None
        } else {
            Some(p.client_secret)
        };
        Self {
            pk: p.pk,
            name: p.name,
            authorization_flow: p.authorization_flow,
            invalidation_flow: p.invalidation_flow,
            client_id: p.client_id,
            client_type: p.client_type,
            redirect_uris: p.redirect_uris,
            client_secret: secret,
        }
    }
}
#[derive(Clone)]
pub struct OAuth2ProviderResource {
    source: AuthentikClientSource,
}
impl OAuth2ProviderResource {
    pub fn new(source: AuthentikClientSource) -> Self {
        Self { source }
    }
}
pub type EnsureOAuth2Provider = EnsureResource<OAuth2ProviderResource>;
pub fn ensure_oauth2_provider(source: AuthentikClientSource) -> EnsureOAuth2Provider {
    EnsureResource::new(OAuth2ProviderResource::new(source))
}

#[async_trait]
impl Resource for OAuth2ProviderResource {
    type Spec = EnsureOAuth2ProviderInput;
    type State = EnsureOAuth2ProviderOutput;
    fn kind(&self) -> &'static str {
        ENSURE_OAUTH2_PROVIDER
    }
    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let client = self.source.client(ctx).await?;
        let found = client
            .oauth2_providers(&spec.name)
            .await
            .map_err(ResourceError::provider)?;
        let Some(summary) = single_exact(
            found,
            |p| p.name == spec.name,
            "OAuth2 provider",
            &spec.name,
        )?
        else {
            return Ok(None);
        };
        Ok(Some(
            client
                .oauth2_provider(summary.pk)
                .await
                .map_err(ResourceError::provider)?
                .unwrap_or(summary)
                .into(),
        ))
    }
    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let body = OAuth2ProviderWrite {
            name: spec.name.clone(),
            authorization_flow: spec.authorization_flow.clone(),
            invalidation_flow: spec.invalidation_flow.clone(),
            client_id: spec.client_id.clone(),
            client_type: spec.client_type.clone(),
            redirect_uris: spec.redirect_uris.clone(),
        };
        let client = self.source.client(ctx).await?;
        let created = client
            .create_oauth2_provider(&body)
            .await
            .map_err(ResourceError::provider)?;
        // GET may include server-generated client secret even if the POST response omits it.
        Ok(client
            .oauth2_provider(created.pk)
            .await
            .map_err(ResourceError::provider)?
            .unwrap_or(created)
            .into())
    }
    fn diff(&self, spec: &Self::Spec, current: &Self::State) -> Drift {
        let mut changed = Vec::new();
        if spec.authorization_flow != current.authorization_flow {
            changed.push("authorization_flow");
        }
        if spec.invalidation_flow != current.invalidation_flow {
            changed.push("invalidation_flow");
        }
        if spec
            .client_id
            .as_ref()
            .is_some_and(|v| v != &current.client_id)
        {
            changed.push("client_id");
        }
        if spec
            .client_type
            .as_ref()
            .is_some_and(|v| v != &current.client_type)
        {
            changed.push("client_type");
        }
        if spec
            .redirect_uris
            .as_ref()
            .is_some_and(|v| !same_set(v, &current.redirect_uris))
        {
            changed.push("redirect_uris");
        }
        drift(changed)
    }
    async fn reconcile(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
        current: Self::State,
    ) -> ResourceResult<Self::State> {
        let mut patch = Map::new();
        patch.insert("authorization_flow".into(), json!(spec.authorization_flow));
        patch.insert("invalidation_flow".into(), json!(spec.invalidation_flow));
        insert_option(&mut patch, "client_id", &spec.client_id);
        insert_option(&mut patch, "client_type", &spec.client_type);
        insert_option(&mut patch, "redirect_uris", &spec.redirect_uris);
        let client = self.source.client(ctx).await?;
        let updated = client
            .patch_oauth2_provider(current.pk, &Value::Object(patch))
            .await
            .map_err(ResourceError::provider)?;
        Ok(client
            .oauth2_provider(current.pk)
            .await
            .map_err(ResourceError::provider)?
            .unwrap_or(updated)
            .into())
    }
}

fn drift(changed: Vec<&str>) -> Drift {
    if changed.is_empty() {
        Drift::InSync
    } else {
        Drift::Drifted(changed.join(", "))
    }
}
fn same_set<T: Ord + Clone>(a: &[T], b: &[T]) -> bool {
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort();
    b.sort();
    a == b
}
fn insert_option<T: Serialize>(map: &mut Map<String, Value>, key: &str, value: &Option<T>) {
    if let Some(v) = value {
        map.insert(key.into(), json!(v));
    }
}
fn single_exact<T>(
    items: Vec<T>,
    mut matches: impl FnMut(&T) -> bool,
    kind: &str,
    key: &str,
) -> ResourceResult<Option<T>> {
    let mut exact = items.into_iter().filter(|item| matches(item));
    let first = exact.next();
    if exact.next().is_some() {
        return Err(ResourceError::provider(format!(
            "multiple Authentik {kind} objects have key {key:?}"
        )));
    }
    Ok(first)
}
async fn application_provider_id(
    client: &AuthentikClient,
    spec: &EnsureApplicationInput,
) -> ResourceResult<Option<i64>> {
    if let Some(name) = &spec.provider_name {
        let found = client
            .oauth2_providers(name)
            .await
            .map_err(ResourceError::provider)?;
        return single_exact(found, |p| p.name == *name, "OAuth2 provider", name)?
            .map(|p| p.pk)
            .ok_or_else(|| {
                ResourceError::provider(format!("Authentik OAuth2 provider {name:?} not found"))
            })
            .map(Some);
    }
    Ok(spec.provider)
}
async fn application_state(
    client: &AuthentikClient,
    app: Application,
    resolve_provider_name: bool,
) -> ResourceResult<EnsureApplicationOutput> {
    let provider_name = if let (true, Some(id)) = (resolve_provider_name, app.provider) {
        client
            .oauth2_provider(id)
            .await
            .map_err(ResourceError::provider)?
            .map(|p| p.name)
    } else {
        None
    };
    Ok(EnsureApplicationOutput {
        pk: app.pk,
        slug: app.slug,
        name: app.name,
        provider: app.provider,
        provider_name,
        meta_description: app.meta_description,
        meta_publisher: app.meta_publisher,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_ext_authentik_api::AuthentikClient;
    use infrazeug_native::{NativeStatus, NodeCtx, NodeMethod, PlanCtx, PlanMethodOutcome};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use uuid::Uuid;

    fn source() -> AuthentikClientSource {
        AuthentikClientSource::ready(AuthentikClient::new("https://auth.example", "token").unwrap())
    }
    #[test]
    fn only_managed_fields_create_drift() {
        let resource = UserResource::new(source());
        let current = User {
            pk: 1,
            username: "alice".into(),
            name: "Alice".into(),
            email: "old@example.com".into(),
            is_active: true,
            path: "users".into(),
            groups: vec!["group-b".into(), "group-a".into()],
        };
        let spec = EnsureUserInput {
            username: "alice".into(),
            name: "Alice".into(),
            ..Default::default()
        };
        assert_eq!(resource.diff(&spec, &current), Drift::InSync);
        let spec = EnsureUserInput {
            email: Some("new@example.com".into()),
            ..spec
        };
        assert!(matches!(resource.diff(&spec, &current), Drift::Drifted(_)));
    }
    #[test]
    fn duplicate_natural_key_is_an_error() {
        assert!(single_exact(vec!["alice", "alice"], |v| *v == "alice", "user", "alice").is_err());
    }
    #[test]
    fn provider_debug_redacts_secret() {
        let output = EnsureOAuth2ProviderOutput {
            pk: 1,
            name: "oidc".into(),
            authorization_flow: "a".into(),
            invalidation_flow: "b".into(),
            client_id: "id".into(),
            client_type: "confidential".into(),
            redirect_uris: vec![],
            client_secret: Some("very-secret".into()),
        };
        assert!(!format!("{output:?}").contains("very-secret"));
        assert_eq!(
            serde_json::to_value(output).unwrap()["client_secret"],
            "very-secret"
        );
    }
    #[tokio::test]
    async fn existing_user_plans_and_executes_without_write() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0_u8; 2048];
                let n = socket.read(&mut buf).await.unwrap();
                requests.push(String::from_utf8_lossy(&buf[..n]).into_owned());
                let body = r#"{"pagination":{"next":0},"results":[{"pk":1,"username":"alice","name":"Alice","is_active":true}]}"#;
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            }
            requests
        });
        let method = ensure_user(AuthentikClientSource::ready(
            AuthentikClient::new(url, "token").unwrap(),
        ));
        let spec = EnsureUserInput {
            username: "alice".into(),
            name: "Alice".into(),
            ..Default::default()
        };
        assert_eq!(
            method
                .plan(&PlanCtx::new(Uuid::nil(), Uuid::nil()), &spec)
                .await
                .unwrap(),
            PlanMethodOutcome::Unchanged
        );
        assert_eq!(
            method
                .execute(&NodeCtx::new(Uuid::nil(), Uuid::nil()), spec)
                .await
                .unwrap()
                .status,
            NativeStatus::Unchanged
        );
        for req in server.await.unwrap() {
            assert!(req.starts_with("GET /api/v3/core/users/"));
        }
    }

    #[tokio::test]
    async fn application_create_then_reconcile_resolves_provider_name() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let app_old = r#"{"pk":"app-uuid","slug":"web","name":"Old","provider":7}"#;
        let app_new = r#"{"pk":"app-uuid","slug":"web","name":"Web","provider":7}"#;
        let provider = r#"{"pk":7,"name":"web-oidc","authorization_flow":"auth-flow","invalidation_flow":"logout-flow"}"#;
        let providers = format!(r#"{{"pagination":{{"next":0}},"results":[{provider}]}}"#);
        let replies = vec![
            (404, r#"{"detail":"Not found"}"#.to_owned()),
            (200, providers.clone()),
            (201, app_old.to_owned()),
            (200, provider.to_owned()),
            (200, app_old.to_owned()),
            (200, provider.to_owned()),
            (200, providers),
            (200, app_new.to_owned()),
            (200, provider.to_owned()),
        ];
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (status, body) in replies {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = Vec::new();
                loop {
                    let mut chunk = [0_u8; 2048];
                    let n = socket.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&buf[..end]);
                        let len = headers
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|v| v.parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if buf.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8_lossy(&buf).into_owned());
                let reason = if status == 404 {
                    "Not Found"
                } else if status == 201 {
                    "Created"
                } else {
                    "OK"
                };
                socket.write_all(format!("HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            }
            requests
        });
        let method = ensure_application(AuthentikClientSource::ready(
            AuthentikClient::new(url, "token").unwrap(),
        ));
        let spec = EnsureApplicationInput {
            slug: "web".into(),
            name: "Old".into(),
            provider_name: Some("web-oidc".into()),
            ..Default::default()
        };
        let node_ctx = NodeCtx::new(Uuid::nil(), Uuid::nil());
        let created = method.execute(&node_ctx, spec.clone()).await.unwrap();
        assert_eq!(created.status, NativeStatus::Changed);
        let captured: Value = serde_json::from_slice(created.capture.as_deref().unwrap()).unwrap();
        assert_eq!(captured["provider"], 7);
        let updated = method
            .execute(
                &node_ctx,
                EnsureApplicationInput {
                    name: "Web".into(),
                    ..spec
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.status, NativeStatus::Changed);
        let requests = server.await.unwrap();
        assert!(requests[2].starts_with("POST /api/v3/core/applications/"));
        assert!(requests[2].contains("\"provider\":7"));
        assert!(requests[7].starts_with("PATCH /api/v3/core/applications/web/"));
        assert!(requests[7].contains("\"provider\":7"));
        assert!(!requests[7].contains("meta_description"));
    }
}
