//! Controller-side Backblaze client construction (environment or vault credentials).

use infrazeug_ext_backblaze_api::{BackblazeClient, BackblazeConfig, Credentials};
use infrazeug_resource::{ResourceCtx, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

const FIELD_APPLICATION_KEY_ID: &str = "application_key_id";
const FIELD_APPLICATION_KEY: &str = "application_key";

const ENV_APPLICATION_KEY_ID: &str = "B2_APPLICATION_KEY_ID";
const ENV_APPLICATION_KEY: &str = "B2_APPLICATION_KEY";

/// Build a [`BackblazeClient`] from standard B2 environment variables.
///
/// Required: `B2_APPLICATION_KEY_ID`, `B2_APPLICATION_KEY`.
pub fn client_from_env() -> anyhow::Result<BackblazeClient> {
    let credentials = Credentials::new(
        std::env::var(ENV_APPLICATION_KEY_ID)
            .map_err(|_| anyhow::anyhow!("{ENV_APPLICATION_KEY_ID} is not set"))?,
        std::env::var(ENV_APPLICATION_KEY)
            .map_err(|_| anyhow::anyhow!("{ENV_APPLICATION_KEY} is not set"))?,
    );
    Ok(BackblazeClient::new(BackblazeConfig::new(credentials)))
}

/// Where a Backblaze resource gets its [`BackblazeClient`].
#[derive(Clone)]
pub enum BackblazeClientSource {
    Ready(Arc<BackblazeClient>),
    Vault {
        file: Arc<str>,
        application_key_id_field: Arc<str>,
        application_key_field: Arc<str>,
        cache: Arc<OnceCell<Arc<BackblazeClient>>>,
    },
}

impl BackblazeClientSource {
    /// A ready client (no vault read).
    pub fn ready(client: BackblazeClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    /// Read credentials from `file` in the controller vault at apply time.
    pub fn vault(file: impl Into<String>) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            application_key_id_field: Arc::from(FIELD_APPLICATION_KEY_ID),
            application_key_field: Arc::from(FIELD_APPLICATION_KEY),
            cache: Arc::new(OnceCell::new()),
        }
    }

    /// Override the vault field for the application key ID.
    pub fn with_application_key_id_field(mut self, field: impl Into<String>) -> Self {
        if let Self::Vault {
            application_key_id_field,
            ..
        } = &mut self
        {
            *application_key_id_field = Arc::from(field.into());
        }
        self
    }

    /// Resolve to a usable client, reading the vault on first use.
    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<BackblazeClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault {
                file,
                application_key_id_field,
                application_key_field,
                cache,
            } => cache
                .get_or_try_init(|| {
                    build_from_vault(ctx, file, application_key_id_field, application_key_field)
                })
                .await
                .cloned(),
        }
    }
}

impl From<BackblazeClient> for BackblazeClientSource {
    fn from(client: BackblazeClient) -> Self {
        Self::ready(client)
    }
}

async fn build_from_vault(
    ctx: &ResourceCtx,
    file: &str,
    application_key_id_field: &str,
    application_key_field: &str,
) -> ResourceResult<Arc<BackblazeClient>> {
    let application_key_id = read_trimmed(ctx, file, application_key_id_field).await?;
    let application_key = read_trimmed(ctx, file, application_key_field).await?;
    Ok(Arc::new(BackblazeClient::new(BackblazeConfig::new(
        Credentials::new(application_key_id, application_key),
    ))))
}

async fn read_trimmed(ctx: &ResourceCtx, file: &str, field: &str) -> ResourceResult<String> {
    Ok(ctx
        .read_secret_string(file, field)
        .await?
        .trim()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_native::NodeCtx;
    use infrazeug_resource::ResourceError;
    use uuid::Uuid;

    fn ctx_without_secrets() -> ResourceCtx {
        ResourceCtx::from(&NodeCtx::new(Uuid::nil(), Uuid::nil()))
    }

    #[tokio::test]
    async fn ready_source_needs_no_vault() {
        let client = BackblazeClient::new(BackblazeConfig::new(Credentials::new("id", "key")));
        let source = BackblazeClientSource::ready(client);
        assert!(source.client(&ctx_without_secrets()).await.is_ok());
    }

    #[tokio::test]
    async fn vault_source_without_vault_is_secrets_unavailable() {
        let source = BackblazeClientSource::vault("cloud/backblaze.vault");
        assert!(matches!(
            source.client(&ctx_without_secrets()).await,
            Err(ResourceError::SecretsUnavailable)
        ));
    }
}
