//! Controller-side OpenStack client construction (ready or vault credentials).

use infrazeug_ext_openstack::{OpenstackClient, OpenstackConfig};
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// Where an OpenStack resource gets its [`OpenstackClient`].
#[derive(Clone)]
pub enum OpenstackClientSource {
    /// Already-authenticated client built up front.
    Ready(Arc<OpenstackClient>),
    /// Keystone username/password read from the controller vault at apply time.
    Vault {
        file: Arc<str>,
        username_field: Arc<str>,
        password_field: Arc<str>,
        config: OpenstackConfig,
        cache: Arc<OnceCell<Arc<OpenstackClient>>>,
    },
}

impl OpenstackClientSource {
    pub fn ready(client: OpenstackClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    pub fn vault(
        file: impl Into<String>,
        username_field: impl Into<String>,
        password_field: impl Into<String>,
        config: OpenstackConfig,
    ) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            username_field: Arc::from(username_field.into()),
            password_field: Arc::from(password_field.into()),
            config,
            cache: Arc::new(OnceCell::new()),
        }
    }

    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<OpenstackClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault {
                file,
                username_field,
                password_field,
                config,
                cache,
            } => cache
                .get_or_try_init(|| {
                    build_from_vault(ctx, file, username_field, password_field, config.clone())
                })
                .await
                .cloned(),
        }
    }
}

impl From<Arc<OpenstackClient>> for OpenstackClientSource {
    fn from(client: Arc<OpenstackClient>) -> Self {
        Self::Ready(client)
    }
}

impl From<OpenstackClient> for OpenstackClientSource {
    fn from(client: OpenstackClient) -> Self {
        Self::ready(client)
    }
}

async fn read_secret_trimmed(ctx: &ResourceCtx, file: &str, field: &str) -> ResourceResult<String> {
    Ok(ctx
        .read_secret_string(file, field)
        .await?
        .trim()
        .to_string())
}

async fn build_from_vault(
    ctx: &ResourceCtx,
    file: &str,
    username_field: &str,
    password_field: &str,
    config: OpenstackConfig,
) -> ResourceResult<Arc<OpenstackClient>> {
    let username = read_secret_trimmed(ctx, file, username_field).await?;
    let password = read_secret_trimmed(ctx, file, password_field).await?;
    let client = OpenstackClient::new(config);
    client
        .authenticate(&username, &password)
        .await
        .map_err(ResourceError::provider)?;
    Ok(Arc::new(client))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infrazeug_native::{NodeCtx, NodeMethod, PlanCtx, PlanMethodOutcome, SecretSource};
    use infrazeug_resource::{EnsureResource, Resource, ResourceResult};
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use uuid::Uuid;

    struct FakeSecrets;

    #[async_trait]
    impl SecretSource for FakeSecrets {
        async fn read_field(&self, file: &str, field: &str) -> infrazeug_native::Result<Vec<u8>> {
            Ok(format!("{file}:{field}").into_bytes())
        }
    }

    #[derive(Clone, Default, Serialize, Deserialize)]
    struct Spec;

    #[derive(Clone, Serialize, Deserialize)]
    struct State {
        id: String,
    }

    #[derive(Clone)]
    struct SecretReader;

    #[async_trait]
    impl Resource for SecretReader {
        type Spec = Spec;
        type State = State;

        fn kind(&self) -> &'static str {
            "openstack.test.secret_reader"
        }

        async fn observe(&self, ctx: &ResourceCtx, _spec: &Spec) -> ResourceResult<Option<State>> {
            ctx.read_secret_string(
                "infra/group_vars/global/vault.vault",
                "vault_ovh_admin_user",
            )
            .await?;
            Ok(None)
        }

        async fn create(&self, _ctx: &ResourceCtx, _spec: &Spec) -> ResourceResult<State> {
            Ok(State {
                id: "created".into(),
            })
        }
    }

    fn plan_ctx() -> PlanCtx {
        PlanCtx::new(Uuid::nil(), Uuid::nil())
    }

    #[tokio::test]
    async fn plan_without_vault_is_unknown() {
        let m = EnsureResource::new(SecretReader);
        assert_eq!(
            m.plan(&plan_ctx(), &Spec).await.unwrap(),
            PlanMethodOutcome::Unknown
        );
    }

    #[tokio::test]
    async fn resource_ctx_reads_secret_from_source() {
        let ctx = NodeCtx::new(Uuid::nil(), Uuid::nil())
            .with_secrets(Some(Arc::new(FakeSecrets) as Arc<dyn SecretSource>));
        let rctx = ResourceCtx::from(&ctx);
        assert!(rctx.has_secrets());
        let value = rctx.read_secret_string("f", "k").await.unwrap();
        assert_eq!(value, "f:k");
    }
}
