use infrazeug_ext_azure_api::{AzureClient, AzureConfig, AzureCredentials};
use infrazeug_resource::{ResourceCtx, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

const FIELD_TENANT_ID: &str = "tenant_id";
const FIELD_CLIENT_ID: &str = "client_id";
const FIELD_CLIENT_SECRET: &str = "client_secret";
const FIELD_SUBSCRIPTION_ID: &str = "subscription_id";

pub fn client_from_env() -> anyhow::Result<AzureClient> {
    Ok(AzureClient::new(AzureConfig::new(AzureCredentials {
        tenant_id: std::env::var("AZURE_TENANT_ID")
            .map_err(|_| anyhow::anyhow!("AZURE_TENANT_ID is not set"))?,
        client_id: std::env::var("AZURE_CLIENT_ID")
            .map_err(|_| anyhow::anyhow!("AZURE_CLIENT_ID is not set"))?,
        client_secret: std::env::var("AZURE_CLIENT_SECRET")
            .map_err(|_| anyhow::anyhow!("AZURE_CLIENT_SECRET is not set"))?,
        subscription_id: std::env::var("AZURE_SUBSCRIPTION_ID")
            .map_err(|_| anyhow::anyhow!("AZURE_SUBSCRIPTION_ID is not set"))?,
    })))
}

#[derive(Clone)]
pub enum AzureClientSource {
    Ready(Arc<AzureClient>),
    Vault {
        file: Arc<str>,
        cache: Arc<OnceCell<Arc<AzureClient>>>,
    },
}

impl AzureClientSource {
    pub fn ready(client: AzureClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    pub fn vault(file: impl Into<String>) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }

    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<AzureClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault { file, cache } => cache
                .get_or_try_init(|| build_from_vault(ctx, file))
                .await
                .cloned(),
        }
    }
}

impl From<AzureClient> for AzureClientSource {
    fn from(client: AzureClient) -> Self {
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

async fn build_from_vault(ctx: &ResourceCtx, file: &str) -> ResourceResult<Arc<AzureClient>> {
    let creds = AzureCredentials {
        tenant_id: read_secret_trimmed(ctx, file, FIELD_TENANT_ID).await?,
        client_id: read_secret_trimmed(ctx, file, FIELD_CLIENT_ID).await?,
        client_secret: read_secret_trimmed(ctx, file, FIELD_CLIENT_SECRET).await?,
        subscription_id: read_secret_trimmed(ctx, file, FIELD_SUBSCRIPTION_ID).await?,
    };
    Ok(Arc::new(AzureClient::new(AzureConfig::new(creds))))
}
