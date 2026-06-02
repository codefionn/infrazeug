//! Controller-side Proxmox client construction (environment or vault credentials).

use infrazeug_ext_proxmox_api::{Auth, ProxmoxClient, ProxmoxConfig};
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

const FIELD_HOST: &str = "host";
const FIELD_TOKEN_ID: &str = "token_id";
const FIELD_TOKEN_SECRET: &str = "token_secret";
const FIELD_USERNAME: &str = "username";
const FIELD_PASSWORD: &str = "password";
const FIELD_INSECURE_TLS: &str = "insecure_tls";

const ENV_HOST: &str = "PROXMOX_HOST";
const ENV_TOKEN_ID: &str = "PROXMOX_TOKEN_ID";
const ENV_TOKEN_SECRET: &str = "PROXMOX_TOKEN_SECRET";
const ENV_USERNAME: &str = "PROXMOX_USERNAME";
const ENV_PASSWORD: &str = "PROXMOX_PASSWORD";
const ENV_INSECURE_TLS: &str = "PROXMOX_INSECURE_TLS";

/// Build a [`ProxmoxClient`] from standard Proxmox environment variables.
///
/// Requires `PROXMOX_HOST` (e.g. `https://pve.example.com:8006`) and either:
/// 1. `PROXMOX_TOKEN_ID` + `PROXMOX_TOKEN_SECRET` (API token, preferred), or
/// 2. `PROXMOX_USERNAME` + `PROXMOX_PASSWORD` (login ticket).
///
/// Optional: `PROXMOX_INSECURE_TLS` (`1`/`true`/`yes`) accepts self-signed certs.
pub fn client_from_env() -> anyhow::Result<ProxmoxClient> {
    let host = std::env::var(ENV_HOST).map_err(|_| anyhow::anyhow!("{ENV_HOST} must be set"))?;
    let auth = if let (Ok(token_id), Ok(secret)) =
        (std::env::var(ENV_TOKEN_ID), std::env::var(ENV_TOKEN_SECRET))
    {
        Auth::api_token(token_id, secret)
    } else {
        let username = std::env::var(ENV_USERNAME).map_err(|_| {
            anyhow::anyhow!(
                "{ENV_TOKEN_ID}+{ENV_TOKEN_SECRET} or {ENV_USERNAME}+{ENV_PASSWORD} must be set"
            )
        })?;
        let password = std::env::var(ENV_PASSWORD)
            .map_err(|_| anyhow::anyhow!("{ENV_PASSWORD} must be set for ticket auth"))?;
        Auth::ticket(username, password)
    };
    let insecure = std::env::var(ENV_INSECURE_TLS)
        .map(|v| is_truthy(&v))
        .unwrap_or(false);
    Ok(ProxmoxClient::new(
        ProxmoxConfig::new(host, auth).insecure_tls(insecure),
    ))
}

/// Where a Proxmox resource gets its [`ProxmoxClient`].
#[derive(Clone)]
pub enum ProxmoxClientSource {
    Ready(Arc<ProxmoxClient>),
    Vault {
        file: Arc<str>,
        cache: Arc<OnceCell<Arc<ProxmoxClient>>>,
    },
}

impl ProxmoxClientSource {
    pub fn ready(client: ProxmoxClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    /// Credentials read from `file` at apply time (`host` + `token_id`/`token_secret`
    /// or `username`/`password`, optional `insecure_tls`).
    pub fn vault(file: impl Into<String>) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }

    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<ProxmoxClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault { file, cache } => cache
                .get_or_try_init(|| build_from_vault(ctx, file))
                .await
                .cloned(),
        }
    }
}

impl From<Arc<ProxmoxClient>> for ProxmoxClientSource {
    fn from(client: Arc<ProxmoxClient>) -> Self {
        Self::Ready(client)
    }
}

impl From<ProxmoxClient> for ProxmoxClientSource {
    fn from(client: ProxmoxClient) -> Self {
        Self::ready(client)
    }
}

async fn build_from_vault(ctx: &ResourceCtx, file: &str) -> ResourceResult<Arc<ProxmoxClient>> {
    // Read `host` first so a missing vault surfaces as `SecretsUnavailable`
    // (treated as "unknown" during a read-only preview) rather than a hard error.
    let host = ctx.read_secret_string(file, FIELD_HOST).await?;

    let auth = match (
        ctx.read_secret_string(file, FIELD_TOKEN_ID).await,
        ctx.read_secret_string(file, FIELD_TOKEN_SECRET).await,
    ) {
        (Ok(token_id), Ok(secret)) => Auth::api_token(token_id, secret),
        (Err(ResourceError::SecretsUnavailable), _)
        | (_, Err(ResourceError::SecretsUnavailable)) => {
            return Err(ResourceError::SecretsUnavailable)
        }
        _ => {
            let username = ctx.read_secret_string(file, FIELD_USERNAME).await?;
            let password = ctx.read_secret_string(file, FIELD_PASSWORD).await?;
            Auth::ticket(username, password)
        }
    };

    let insecure = match ctx.read_secret_string(file, FIELD_INSECURE_TLS).await {
        Ok(value) => is_truthy(&value),
        Err(ResourceError::SecretsUnavailable) => return Err(ResourceError::SecretsUnavailable),
        Err(_) => false,
    };

    Ok(Arc::new(ProxmoxClient::new(
        ProxmoxConfig::new(host, auth).insecure_tls(insecure),
    )))
}

fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
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
        let client = ProxmoxClient::new(ProxmoxConfig::new(
            "https://pve.example.com:8006",
            Auth::api_token("root@pam!ci", "secret"),
        ));
        let source = ProxmoxClientSource::ready(client);
        assert!(source.client(&ctx_without_secrets()).await.is_ok());
    }

    #[tokio::test]
    async fn vault_source_without_vault_is_secrets_unavailable() {
        let source = ProxmoxClientSource::vault("cloud/proxmox.vault");
        assert!(matches!(
            source.client(&ctx_without_secrets()).await,
            Err(ResourceError::SecretsUnavailable)
        ));
    }

    #[test]
    fn truthy_parsing() {
        assert!(is_truthy("1"));
        assert!(is_truthy(" TRUE "));
        assert!(is_truthy("yes"));
        assert!(!is_truthy("0"));
        assert!(!is_truthy("no"));
    }
}
