//! Router-side MikroTik client construction (environment or vault credentials).

use infrazeug_ext_mikrotik_api::{Credentials, MikrotikClient, MikrotikConfig};
use infrazeug_resource::{ResourceCtx, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

const FIELD_USERNAME: &str = "username";
const FIELD_PASSWORD: &str = "password";

const ENV_HOST: &str = "MIKROTIK_HOST";
const ENV_USERNAME: &str = "MIKROTIK_USERNAME";
const ENV_PASSWORD: &str = "MIKROTIK_PASSWORD";
const ENV_PORT: &str = "MIKROTIK_PORT";
const ENV_TLS: &str = "MIKROTIK_TLS";
const ENV_INSECURE: &str = "MIKROTIK_INSECURE";

/// Resolved connection parameters (fresh TCP session per API call).
#[derive(Clone)]
pub struct MikrotikParams {
    pub config: MikrotikConfig,
    pub credentials: Credentials,
}

impl MikrotikParams {
    /// Open a connected API client.
    pub async fn connect(&self) -> infrazeug_ext_mikrotik_api::Result<MikrotikClient> {
        MikrotikClient::open(self.config.clone(), self.credentials.clone()).await
    }

    /// Override the API port.
    pub fn with_port(mut self, port: u16) -> Self {
        self.config = self.config.with_port(port);
        self
    }

    /// Use API-SSL (port 8729 unless overridden).
    pub fn with_tls(mut self, tls: bool) -> Self {
        self.config = self.config.with_tls(tls);
        self
    }

    /// Toggle acceptance of invalid TLS certificates (API-SSL only).
    ///
    /// Verification is on by default; pass `true` to ignore the router's
    /// self-signed certificate.
    pub fn with_accept_invalid_certs(mut self, accept: bool) -> Self {
        self.config = self.config.with_accept_invalid_certs(accept);
        self
    }

    /// Skip TLS certificate verification for API-SSL. Shorthand for
    /// `with_accept_invalid_certs(true)`.
    ///
    /// Pair with [`with_tls`](Self::with_tls)(`true`) when using port 8729 —
    /// plain TCP on 8728 is unaffected.
    pub fn insecure(self) -> Self {
        self.with_accept_invalid_certs(true)
    }
}

/// Build [`MikrotikParams`] from standard environment variables.
///
/// Required: `MIKROTIK_HOST`, `MIKROTIK_USERNAME`, `MIKROTIK_PASSWORD`.
///
/// Optional: `MIKROTIK_PORT`, `MIKROTIK_TLS` (truthy → API-SSL on 8729),
/// `MIKROTIK_INSECURE` (truthy → skip TLS verification on API-SSL).
///
/// RouterOS ships a self-signed certificate for API-SSL; against a stock router
/// you typically need `MIKROTIK_TLS=1` and `MIKROTIK_INSECURE=1`. Plain API on
/// 8728 needs neither (traffic is unencrypted at the transport layer).
pub fn client_from_env() -> anyhow::Result<MikrotikParams> {
    let host = std::env::var(ENV_HOST).map_err(|_| anyhow::anyhow!("{ENV_HOST} is not set"))?;
    let username =
        std::env::var(ENV_USERNAME).map_err(|_| anyhow::anyhow!("{ENV_USERNAME} is not set"))?;
    let password =
        std::env::var(ENV_PASSWORD).map_err(|_| anyhow::anyhow!("{ENV_PASSWORD} is not set"))?;

    let mut config = MikrotikConfig::new(host);
    if let Ok(port) = std::env::var(ENV_PORT) {
        config = config.with_port(
            port.parse()
                .map_err(|_| anyhow::anyhow!("invalid {ENV_PORT}"))?,
        );
    }
    if env_truthy(ENV_TLS) {
        config = config.with_tls(true);
    }
    if env_truthy(ENV_INSECURE) {
        config = config.with_accept_invalid_certs(true);
    }

    Ok(MikrotikParams {
        config,
        credentials: Credentials::new(username, password),
    })
}

fn env_truthy(name: &str) -> bool {
    std::env::var(name)
        .map(|v| !matches!(v.trim(), "" | "0" | "false" | "no"))
        .unwrap_or(false)
}

/// Where a MikroTik resource gets its connection parameters.
#[derive(Clone)]
pub enum MikrotikClientSource {
    Ready(Arc<MikrotikParams>),
    Vault {
        host: Arc<str>,
        port: u16,
        tls: bool,
        accept_invalid_certs: bool,
        file: Arc<str>,
        username_field: Arc<str>,
        password_field: Arc<str>,
        cache: Arc<OnceCell<Arc<MikrotikParams>>>,
    },
}

impl MikrotikClientSource {
    /// Pre-resolved parameters (no vault read).
    pub fn ready(params: MikrotikParams) -> Self {
        Self::Ready(Arc::new(params))
    }

    /// Read `username` / `password` from `file` in the controller vault at apply time.
    pub fn vault(host: impl Into<String>, file: impl Into<String>) -> Self {
        Self::vault_fields(host, file, FIELD_USERNAME, FIELD_PASSWORD)
    }

    /// Like [`vault`](Self::vault) with explicit credential field names.
    pub fn vault_fields(
        host: impl Into<String>,
        file: impl Into<String>,
        username_field: impl Into<String>,
        password_field: impl Into<String>,
    ) -> Self {
        Self::Vault {
            host: Arc::from(host.into()),
            port: infrazeug_ext_mikrotik_api::DEFAULT_PLAIN_PORT,
            tls: false,
            accept_invalid_certs: false,
            file: Arc::from(file.into()),
            username_field: Arc::from(username_field.into()),
            password_field: Arc::from(password_field.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        if let Self::Vault { port: p, .. } = &mut self {
            *p = port;
        }
        self
    }

    pub fn with_tls(mut self, tls: bool) -> Self {
        if let Self::Vault { tls: t, port, .. } = &mut self {
            *t = tls;
            if tls && *port == infrazeug_ext_mikrotik_api::DEFAULT_PLAIN_PORT {
                *port = infrazeug_ext_mikrotik_api::DEFAULT_TLS_PORT;
            }
        }
        self
    }

    pub fn with_accept_invalid_certs(mut self, accept: bool) -> Self {
        if let Self::Vault {
            accept_invalid_certs,
            ..
        } = &mut self
        {
            *accept_invalid_certs = accept;
        }
        self
    }

    /// Skip TLS certificate verification for a vault source (ignore the router's
    /// self-signed API-SSL certificate). Shorthand for `with_accept_invalid_certs(true)`.
    /// No-op on a ready source (configure its [`MikrotikConfig`] / [`MikrotikParams`] instead).
    pub fn insecure(self) -> Self {
        self.with_accept_invalid_certs(true)
    }

    /// Resolve connection parameters, reading the vault on first use.
    pub async fn params(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<MikrotikParams>> {
        match self {
            Self::Ready(params) => Ok(Arc::clone(params)),
            Self::Vault {
                host,
                port,
                tls,
                accept_invalid_certs,
                file,
                username_field,
                password_field,
                cache,
            } => cache
                .get_or_try_init(|| {
                    build_from_vault(
                        ctx,
                        host,
                        *port,
                        *tls,
                        *accept_invalid_certs,
                        file,
                        username_field,
                        password_field,
                    )
                })
                .await
                .cloned(),
        }
    }
}

impl From<MikrotikParams> for MikrotikClientSource {
    fn from(params: MikrotikParams) -> Self {
        Self::ready(params)
    }
}

#[allow(clippy::too_many_arguments)]
async fn build_from_vault(
    ctx: &ResourceCtx,
    host: &str,
    port: u16,
    tls: bool,
    accept_invalid_certs: bool,
    file: &str,
    username_field: &str,
    password_field: &str,
) -> ResourceResult<Arc<MikrotikParams>> {
    let username = read_trimmed(ctx, file, username_field).await?;
    let password = read_trimmed(ctx, file, password_field).await?;
    let mut config = MikrotikConfig::new(host.to_string()).with_port(port);
    if tls {
        config = config.with_tls(true);
    }
    config = config.with_accept_invalid_certs(accept_invalid_certs);
    Ok(Arc::new(MikrotikParams {
        config,
        credentials: Credentials::new(username, password),
    }))
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
        let params = MikrotikParams {
            config: MikrotikConfig::new("192.168.88.1"),
            credentials: Credentials::new("admin", "pw"),
        };
        let source = MikrotikClientSource::ready(params);
        assert!(source.params(&ctx_without_secrets()).await.is_ok());
    }

    #[tokio::test]
    async fn vault_source_without_vault_is_secrets_unavailable() {
        let source = MikrotikClientSource::vault("192.168.88.1", "cloud/mikrotik.vault");
        assert!(matches!(
            source.params(&ctx_without_secrets()).await,
            Err(ResourceError::SecretsUnavailable)
        ));
    }

    #[test]
    fn params_insecure_sets_accept_invalid_certs() {
        let params = MikrotikParams {
            config: MikrotikConfig::new("192.168.88.1").with_tls(true),
            credentials: Credentials::new("admin", "pw"),
        }
        .insecure();
        assert!(params.config.tls);
        assert!(params.config.accept_invalid_certs);
    }

    #[test]
    fn vault_setters_apply() {
        let source = MikrotikClientSource::vault("192.168.88.1", "f")
            .with_port(8729)
            .with_tls(true)
            .insecure();
        match source {
            MikrotikClientSource::Vault {
                port,
                tls,
                accept_invalid_certs,
                ..
            } => {
                assert_eq!(port, 8729);
                assert!(tls);
                assert!(accept_invalid_certs);
            }
            _ => panic!("expected vault source"),
        }
    }
}
