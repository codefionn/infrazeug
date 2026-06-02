//! Controller-side AWS client construction (environment or vault credentials).

use infrazeug_ext_aws_api::{AwsClient, AwsConfig, AwsCredentials};
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

const FIELD_ACCESS_KEY_ID: &str = "access_key_id";
const FIELD_SECRET_ACCESS_KEY: &str = "secret_access_key";
const FIELD_SESSION_TOKEN: &str = "session_token";
const FIELD_REGION: &str = "region";

/// Build an [`AwsClient`] from standard AWS environment variables.
///
/// Required: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`.
/// Optional: `AWS_SESSION_TOKEN`.
pub fn client_from_env() -> anyhow::Result<AwsClient> {
    let credentials = AwsCredentials::new(
        std::env::var("AWS_ACCESS_KEY_ID")
            .map_err(|_| anyhow::anyhow!("AWS_ACCESS_KEY_ID is not set"))?,
        std::env::var("AWS_SECRET_ACCESS_KEY")
            .map_err(|_| anyhow::anyhow!("AWS_SECRET_ACCESS_KEY is not set"))?,
    );
    let credentials = if let Ok(token) = std::env::var("AWS_SESSION_TOKEN") {
        credentials.with_session_token(token)
    } else {
        credentials
    };
    let region = std::env::var("AWS_REGION")
        .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
        .unwrap_or_else(|_| "us-east-1".into());
    Ok(AwsClient::new(AwsConfig::new(credentials, region)))
}

/// Where an AWS resource gets its [`AwsClient`].
#[derive(Clone)]
pub enum AwsClientSource {
    Ready(Arc<AwsClient>),
    Vault {
        file: Arc<str>,
        cache: Arc<OnceCell<Arc<AwsClient>>>,
    },
}

impl AwsClientSource {
    pub fn ready(client: AwsClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    pub fn vault(file: impl Into<String>) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }

    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<AwsClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault { file, cache } => cache
                .get_or_try_init(|| build_from_vault(ctx, file))
                .await
                .cloned(),
        }
    }
}

impl From<Arc<AwsClient>> for AwsClientSource {
    fn from(client: Arc<AwsClient>) -> Self {
        Self::Ready(client)
    }
}

impl From<AwsClient> for AwsClientSource {
    fn from(client: AwsClient) -> Self {
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

async fn build_from_vault(ctx: &ResourceCtx, file: &str) -> ResourceResult<Arc<AwsClient>> {
    let access_key_id = read_secret_trimmed(ctx, file, FIELD_ACCESS_KEY_ID).await?;
    let secret_access_key = read_secret_trimmed(ctx, file, FIELD_SECRET_ACCESS_KEY).await?;
    let mut credentials = AwsCredentials::new(access_key_id, secret_access_key);
    if let Ok(token) = ctx.read_secret_string(file, FIELD_SESSION_TOKEN).await {
        credentials = credentials.with_session_token(token.trim());
    }
    let region = match ctx.read_secret_string(file, FIELD_REGION).await {
        Ok(r) => r.trim().to_string(),
        Err(ResourceError::SecretsUnavailable) => return Err(ResourceError::SecretsUnavailable),
        Err(_) => "us-east-1".into(),
    };
    Ok(Arc::new(AwsClient::new(AwsConfig::new(
        credentials,
        region,
    ))))
}
