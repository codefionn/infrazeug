//! Controller-side IONOS client construction (environment or vault credentials).

use infrazeug_ext_ionos_cloud_api::{Auth, IonosClient, IonosConfig};
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// Default vault field names read by [`IonosClientSource::Vault`].
const FIELD_TOKEN: &str = "token";
const FIELD_CONTRACT_NUMBER: &str = "contract_number";

/// Build an [`IonosClient`] from standard IONOS environment variables.
///
/// Required: `IONOS_TOKEN`. Optional: `IONOS_CONTRACT_NUMBER` (multi-contract
/// accounts).
pub fn client_from_env() -> anyhow::Result<IonosClient> {
    let token =
        std::env::var("IONOS_TOKEN").map_err(|_| anyhow::anyhow!("IONOS_TOKEN is not set"))?;
    let mut config = IonosConfig::new(Auth::token(token));
    if let Ok(contract) = std::env::var("IONOS_CONTRACT_NUMBER") {
        config = config.with_contract_number(contract);
    }
    Ok(IonosClient::new(config))
}

/// Where an IONOS resource gets its [`IonosClient`].
///
/// - [`Ready`](Self::Ready): a client built up front (e.g. from [`client_from_env`]).
/// - [`Vault`](Self::Vault): the API token read from the controller's unlocked vault
///   at apply time, so no `IONOS_*` secrets need to live in the environment. The
///   native node runs on the controller, which already holds the unlocked vault
///   session, so the client is built lazily inside `observe`/`create` and cached.
#[derive(Clone)]
pub enum IonosClientSource {
    Ready(Arc<IonosClient>),
    Vault {
        /// Vault file (under `files/`) holding the credential fields.
        file: Arc<str>,
        /// Field holding the IONOS API token.
        token_field: Arc<str>,
        /// Field holding an optional contract number (multi-contract accounts).
        contract_field: Arc<str>,
        /// Built once per run, shared across the resources of one builder.
        cache: Arc<OnceCell<Arc<IonosClient>>>,
    },
}

impl IonosClientSource {
    /// A ready client (no vault read).
    pub fn ready(client: IonosClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    /// Token read from `file` in the controller vault at apply time, using the
    /// default field names (`token`, optional `contract_number`).
    pub fn vault(file: impl Into<String>) -> Self {
        Self::vault_fields(file, FIELD_TOKEN, FIELD_CONTRACT_NUMBER)
    }

    /// Like [`vault`](Self::vault) with explicit field names, so existing vault
    /// fields can be reused without re-sealing.
    pub fn vault_fields(
        file: impl Into<String>,
        token_field: impl Into<String>,
        contract_field: impl Into<String>,
    ) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            token_field: Arc::from(token_field.into()),
            contract_field: Arc::from(contract_field.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }

    /// Resolve to a usable client, reading the vault on first use.
    ///
    /// Returns [`ResourceError::SecretsUnavailable`] when a vault-backed source is used
    /// without an unlocked controller vault (e.g. read-only preview); callers in the
    /// plan path treat that as "unknown" rather than a failure.
    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<IonosClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault {
                file,
                token_field,
                contract_field,
                cache,
            } => cache
                .get_or_try_init(|| build_from_vault(ctx, file, token_field, contract_field))
                .await
                .cloned(),
        }
    }
}

impl From<Arc<IonosClient>> for IonosClientSource {
    fn from(client: Arc<IonosClient>) -> Self {
        Self::Ready(client)
    }
}

impl From<IonosClient> for IonosClientSource {
    fn from(client: IonosClient) -> Self {
        Self::ready(client)
    }
}

async fn build_from_vault(
    ctx: &ResourceCtx,
    file: &str,
    token_field: &str,
    contract_field: &str,
) -> ResourceResult<Arc<IonosClient>> {
    let token = ctx.read_secret_string(file, token_field).await?;
    let mut config = IonosConfig::new(Auth::token(token));
    // The contract number is optional; absent is fine, only a missing vault is fatal.
    match ctx.read_secret_string(file, contract_field).await {
        Ok(contract) => config = config.with_contract_number(contract),
        Err(ResourceError::SecretsUnavailable) => return Err(ResourceError::SecretsUnavailable),
        Err(_) => {}
    }
    Ok(Arc::new(IonosClient::new(config)))
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
        let client = IonosClient::new(IonosConfig::new(Auth::token("t")));
        let source = IonosClientSource::ready(client);
        assert!(source.client(&ctx_without_secrets()).await.is_ok());
    }

    #[tokio::test]
    async fn vault_source_without_vault_is_secrets_unavailable() {
        let source = IonosClientSource::vault("cloud/ionos.vault");
        assert!(matches!(
            source.client(&ctx_without_secrets()).await,
            Err(ResourceError::SecretsUnavailable)
        ));
    }
}
