//! Controller-side Cloudflare client construction (environment or vault credentials).

use infrazeug_ext_cloudflare_api::{Auth, CloudflareClient, CloudflareConfig};
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

const FIELD_API_TOKEN: &str = "api_token";
const FIELD_EMAIL: &str = "email";
const FIELD_API_KEY: &str = "api_key";

const ENV_API_TOKEN: &str = "CLOUDFLARE_API_TOKEN";
const ENV_EMAIL: &str = "CLOUDFLARE_EMAIL";
const ENV_API_KEY: &str = "CLOUDFLARE_API_KEY";
const ENV_ACCOUNT_ID: &str = "CLOUDFLARE_ACCOUNT_ID";

const FIELD_ACCOUNT_ID: &str = "account_id";

/// Build a [`CloudflareClient`] from standard Cloudflare environment variables.
///
/// Preferred: `CLOUDFLARE_API_TOKEN` (scoped API token).
///
/// Legacy: `CLOUDFLARE_EMAIL` + `CLOUDFLARE_API_KEY` (global API key).
pub fn client_from_env() -> anyhow::Result<CloudflareClient> {
    let auth = if let Ok(token) = std::env::var(ENV_API_TOKEN) {
        Auth::token(token)
    } else {
        let email = std::env::var(ENV_EMAIL)
            .map_err(|_| anyhow::anyhow!("set {ENV_API_TOKEN}, or {ENV_EMAIL} + {ENV_API_KEY}"))?;
        let api_key = std::env::var(ENV_API_KEY)
            .map_err(|_| anyhow::anyhow!("set {ENV_API_TOKEN}, or {ENV_EMAIL} + {ENV_API_KEY}"))?;
        Auth::global_key(email, api_key)
    };
    Ok(CloudflareClient::new(config_from_env(auth)))
}

fn config_from_env(auth: Auth) -> CloudflareConfig {
    let mut config = CloudflareConfig::new(auth);
    if let Ok(account_id) = std::env::var(ENV_ACCOUNT_ID) {
        let account_id = account_id.trim();
        if !account_id.is_empty() {
            config = config.with_account_id(account_id);
        }
    }
    config
}

/// Where a Cloudflare resource gets its [`CloudflareClient`].
///
/// - [`Ready`](Self::Ready): a client built up front (e.g. from [`client_from_env`]).
/// - [`Vault`](Self::Vault): credentials read from the controller's unlocked vault
///   at apply time. An `api_token` field is tried first; if absent, `email` /
///   `api_key` are used for global-key auth.
#[derive(Clone)]
pub enum CloudflareClientSource {
    Ready(Arc<CloudflareClient>),
    Vault {
        file: Arc<str>,
        api_token_field: Arc<str>,
        email_field: Arc<str>,
        api_key_field: Arc<str>,
        account_id_field: Arc<str>,
        cache: Arc<OnceCell<Arc<CloudflareClient>>>,
    },
}

impl CloudflareClientSource {
    /// A ready client (no vault read).
    pub fn ready(client: CloudflareClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    /// Read credentials from `file` in the controller vault at apply time.
    pub fn vault(file: impl Into<String>) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            api_token_field: Arc::from(FIELD_API_TOKEN),
            email_field: Arc::from(FIELD_EMAIL),
            api_key_field: Arc::from(FIELD_API_KEY),
            account_id_field: Arc::from(FIELD_ACCOUNT_ID),
            cache: Arc::new(OnceCell::new()),
        }
    }

    /// Override the vault field checked for an account id (default `account_id`).
    pub fn with_account_id_field(mut self, field: impl Into<String>) -> Self {
        if let Self::Vault {
            account_id_field, ..
        } = &mut self
        {
            *account_id_field = Arc::from(field.into());
        }
        self
    }

    /// Override the vault field checked for an API token (default `api_token`).
    pub fn with_api_token_field(mut self, field: impl Into<String>) -> Self {
        if let Self::Vault {
            api_token_field, ..
        } = &mut self
        {
            *api_token_field = Arc::from(field.into());
        }
        self
    }

    /// Resolve to a usable client, reading the vault on first use.
    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<CloudflareClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault {
                file,
                api_token_field,
                email_field,
                api_key_field,
                account_id_field,
                cache,
            } => cache
                .get_or_try_init(|| {
                    build_from_vault(
                        ctx,
                        file,
                        api_token_field,
                        email_field,
                        api_key_field,
                        account_id_field,
                    )
                })
                .await
                .cloned(),
        }
    }
}

impl From<CloudflareClient> for CloudflareClientSource {
    fn from(client: CloudflareClient) -> Self {
        Self::ready(client)
    }
}

async fn build_from_vault(
    ctx: &ResourceCtx,
    file: &str,
    api_token_field: &str,
    email_field: &str,
    api_key_field: &str,
    account_id_field: &str,
) -> ResourceResult<Arc<CloudflareClient>> {
    let auth = match read_optional(ctx, file, api_token_field).await? {
        Some(token) => Auth::token(token),
        None => {
            let email = read_trimmed(ctx, file, email_field).await?;
            let api_key = read_trimmed(ctx, file, api_key_field).await?;
            Auth::global_key(email, api_key)
        }
    };
    let mut config = CloudflareConfig::new(auth);
    if let Some(account_id) = read_optional(ctx, file, account_id_field).await? {
        let account_id = account_id.trim();
        if !account_id.is_empty() {
            config = config.with_account_id(account_id);
        }
    }
    Ok(Arc::new(CloudflareClient::new(config)))
}

async fn read_trimmed(ctx: &ResourceCtx, file: &str, field: &str) -> ResourceResult<String> {
    Ok(ctx
        .read_secret_string(file, field)
        .await?
        .trim()
        .to_string())
}

async fn read_optional(
    ctx: &ResourceCtx,
    file: &str,
    field: &str,
) -> ResourceResult<Option<String>> {
    match ctx.read_secret_string(file, field).await {
        Ok(value) => Ok(Some(value.trim().to_string())),
        Err(ResourceError::SecretsUnavailable) => Err(ResourceError::SecretsUnavailable),
        Err(_) => Ok(None),
    }
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
        let client = CloudflareClient::new(CloudflareConfig::new(Auth::token("t")));
        let source = CloudflareClientSource::ready(client);
        assert!(source.client(&ctx_without_secrets()).await.is_ok());
    }

    #[tokio::test]
    async fn vault_source_without_vault_is_secrets_unavailable() {
        let source = CloudflareClientSource::vault("cloud/cloudflare.vault");
        assert!(matches!(
            source.client(&ctx_without_secrets()).await,
            Err(ResourceError::SecretsUnavailable)
        ));
    }
}
