//! Controller-side STACKIT client construction (environment or vault credentials).

use infrazeug_ext_stackit_api::{Auth, StackitClient, StackitConfig};
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

const FIELD_TOKEN: &str = "token";
const FIELD_SERVICE_ACCOUNT_KEY: &str = "service_account_key";
const FIELD_PRIVATE_KEY: &str = "private_key";
const ENV_TOKEN: &str = "STACKIT_SERVICE_ACCOUNT_TOKEN";
const ENV_SERVICE_ACCOUNT_KEY: &str = "STACKIT_SERVICE_ACCOUNT_KEY";
const ENV_PRIVATE_KEY: &str = "STACKIT_PRIVATE_KEY";
const ENV_HOST: &str = "STACKIT_IAAS_HOST";

/// Build a [`StackitClient`] from standard STACKIT environment variables.
///
/// Tries, in order:
/// 1. `STACKIT_SERVICE_ACCOUNT_TOKEN` (token flow)
/// 2. `STACKIT_SERVICE_ACCOUNT_KEY` + optional `STACKIT_PRIVATE_KEY` (key flow)
///
/// Optional: `STACKIT_IAAS_HOST` overrides the regional API host.
pub fn client_from_env() -> anyhow::Result<StackitClient> {
    let mut config = if let Ok(token) = std::env::var(ENV_TOKEN) {
        StackitConfig::new(Auth::token(token))
    } else {
        let key_json = std::env::var(ENV_SERVICE_ACCOUNT_KEY)
            .map_err(|_| anyhow::anyhow!("{ENV_TOKEN} or {ENV_SERVICE_ACCOUNT_KEY} must be set"))?;
        let private_key = std::env::var(ENV_PRIVATE_KEY).ok();
        let auth = Auth::service_account_key_json(&key_json, private_key)?;
        StackitConfig::new(auth)
    };
    if let Ok(host) = std::env::var(ENV_HOST) {
        config = config.with_host(host);
    }
    Ok(StackitClient::new(config))
}

/// Where a STACKIT resource gets its [`StackitClient`].
#[derive(Clone)]
pub enum StackitClientSource {
    Ready(Arc<StackitClient>),
    Vault {
        file: Arc<str>,
        token_field: Arc<str>,
        service_account_key_field: Arc<str>,
        private_key_field: Arc<str>,
        cache: Arc<OnceCell<Arc<StackitClient>>>,
    },
}

impl StackitClientSource {
    pub fn ready(client: StackitClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    /// Token read from `file` at apply time (`token` field by default).
    pub fn vault(file: impl Into<String>) -> Self {
        Self::vault_fields(
            file,
            FIELD_TOKEN,
            FIELD_SERVICE_ACCOUNT_KEY,
            FIELD_PRIVATE_KEY,
        )
    }

    pub fn vault_fields(
        file: impl Into<String>,
        token_field: impl Into<String>,
        service_account_key_field: impl Into<String>,
        private_key_field: impl Into<String>,
    ) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            token_field: Arc::from(token_field.into()),
            service_account_key_field: Arc::from(service_account_key_field.into()),
            private_key_field: Arc::from(private_key_field.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }

    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<StackitClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault {
                file,
                token_field,
                service_account_key_field,
                private_key_field,
                cache,
            } => cache
                .get_or_try_init(|| {
                    build_from_vault(
                        ctx,
                        file,
                        token_field,
                        service_account_key_field,
                        private_key_field,
                    )
                })
                .await
                .cloned(),
        }
    }
}

impl From<Arc<StackitClient>> for StackitClientSource {
    fn from(client: Arc<StackitClient>) -> Self {
        Self::Ready(client)
    }
}

impl From<StackitClient> for StackitClientSource {
    fn from(client: StackitClient) -> Self {
        Self::ready(client)
    }
}

async fn build_from_vault(
    ctx: &ResourceCtx,
    file: &str,
    token_field: &str,
    service_account_key_field: &str,
    private_key_field: &str,
) -> ResourceResult<Arc<StackitClient>> {
    if let Ok(token) = ctx.read_secret_string(file, token_field).await {
        return Ok(Arc::new(StackitClient::new(StackitConfig::new(
            Auth::token(token),
        ))));
    }

    let key_json = ctx
        .read_secret_string(file, service_account_key_field)
        .await?;
    let private_key = match ctx.read_secret_string(file, private_key_field).await {
        Ok(key) => Some(key),
        Err(ResourceError::SecretsUnavailable) => return Err(ResourceError::SecretsUnavailable),
        Err(_) => None,
    };
    let auth =
        Auth::service_account_key_json(&key_json, private_key).map_err(ResourceError::provider)?;
    Ok(Arc::new(StackitClient::new(StackitConfig::new(auth))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_native::NodeCtx;
    use uuid::Uuid;

    fn ctx_without_secrets() -> ResourceCtx {
        ResourceCtx::from(&NodeCtx::new(Uuid::nil(), Uuid::nil()))
    }

    #[tokio::test]
    async fn ready_source_needs_no_vault() {
        let client = StackitClient::new(StackitConfig::new(Auth::token("t")));
        let source = StackitClientSource::ready(client);
        assert!(source.client(&ctx_without_secrets()).await.is_ok());
    }

    #[tokio::test]
    async fn vault_source_without_vault_is_secrets_unavailable() {
        let source = StackitClientSource::vault("cloud/stackit.vault");
        assert!(matches!(
            source.client(&ctx_without_secrets()).await,
            Err(ResourceError::SecretsUnavailable)
        ));
    }
}
