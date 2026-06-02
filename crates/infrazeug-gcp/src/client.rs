//! Controller-side GCP client construction (environment or vault credentials).

use infrazeug_ext_gcp_api::{GcpAuth, GcpClient, GcpConfig, ServiceAccountKey};
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

const FIELD_SERVICE_ACCOUNT_JSON: &str = "service_account_json";

/// Build a [`GcpClient`] from `GOOGLE_APPLICATION_CREDENTIALS` or `GCP_SERVICE_ACCOUNT_JSON`.
pub fn client_from_env() -> anyhow::Result<GcpClient> {
    let json = if let Ok(path) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
        std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("read GOOGLE_APPLICATION_CREDENTIALS: {e}"))?
    } else {
        std::env::var("GCP_SERVICE_ACCOUNT_JSON").map_err(|_| {
            anyhow::anyhow!("GOOGLE_APPLICATION_CREDENTIALS or GCP_SERVICE_ACCOUNT_JSON is not set")
        })?
    };
    let key = ServiceAccountKey::from_json(&json)
        .map_err(|e| anyhow::anyhow!("parse service account json: {e}"))?;
    Ok(GcpClient::new(GcpConfig::new(GcpAuth::new(key))))
}

#[derive(Clone)]
pub enum GcpClientSource {
    Ready(Arc<GcpClient>),
    Vault {
        file: Arc<str>,
        field: Arc<str>,
        cache: Arc<OnceCell<Arc<GcpClient>>>,
    },
}

impl GcpClientSource {
    pub fn ready(client: GcpClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    pub fn vault(file: impl Into<String>) -> Self {
        Self::vault_field(file, FIELD_SERVICE_ACCOUNT_JSON)
    }

    pub fn vault_field(file: impl Into<String>, field: impl Into<String>) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            field: Arc::from(field.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }

    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<GcpClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault { file, field, cache } => cache
                .get_or_try_init(|| build_from_vault(ctx, file, field))
                .await
                .cloned(),
        }
    }
}

impl From<GcpClient> for GcpClientSource {
    fn from(client: GcpClient) -> Self {
        Self::ready(client)
    }
}

async fn build_from_vault(
    ctx: &ResourceCtx,
    file: &str,
    field: &str,
) -> ResourceResult<Arc<GcpClient>> {
    let json = ctx.read_secret_string(file, field).await?;
    let key = ServiceAccountKey::from_json(&json).map_err(ResourceError::provider)?;
    Ok(Arc::new(GcpClient::new(GcpConfig::new(GcpAuth::new(key)))))
}
