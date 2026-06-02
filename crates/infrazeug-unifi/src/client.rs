//! Controller-side UniFi client construction (environment or vault credentials).

use infrazeug_ext_unifi_api::{ControllerKind, Credentials, UnifiClient, UnifiConfig};
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

const FIELD_API_KEY: &str = "api_key";
const FIELD_USERNAME: &str = "username";
const FIELD_PASSWORD: &str = "password";

const ENV_HOST: &str = "UNIFI_HOST";
const ENV_USERNAME: &str = "UNIFI_USERNAME";
const ENV_PASSWORD: &str = "UNIFI_PASSWORD";
const ENV_API_KEY: &str = "UNIFI_API_KEY";
const ENV_SITE: &str = "UNIFI_SITE";
const ENV_CONTROLLER: &str = "UNIFI_CONTROLLER";
const ENV_INSECURE: &str = "UNIFI_INSECURE";

/// Build a [`UnifiClient`] from standard UniFi environment variables.
///
/// Required: `UNIFI_HOST` plus either `UNIFI_API_KEY` (key auth) or
/// `UNIFI_USERNAME` + `UNIFI_PASSWORD` (session login).
///
/// Optional: `UNIFI_SITE` (default `default`), `UNIFI_CONTROLLER`
/// (`unifios` [default] or `legacy`), and `UNIFI_INSECURE` (set truthy to ignore
/// the controller's TLS certificate; verification is **on** by default — UniFi's
/// stock self-signed certificate needs `UNIFI_INSECURE=1`).
pub fn client_from_env() -> anyhow::Result<UnifiClient> {
    let host = std::env::var(ENV_HOST).map_err(|_| anyhow::anyhow!("{ENV_HOST} is not set"))?;

    let credentials = if let Ok(key) = std::env::var(ENV_API_KEY) {
        Credentials::api_key(key)
    } else if let (Ok(username), Ok(password)) =
        (std::env::var(ENV_USERNAME), std::env::var(ENV_PASSWORD))
    {
        Credentials::user_pass(username, password)
    } else {
        anyhow::bail!("set {ENV_API_KEY}, or {ENV_USERNAME} + {ENV_PASSWORD}");
    };

    let mut config = UnifiConfig::new(host, credentials);
    if let Ok(site) = std::env::var(ENV_SITE) {
        config = config.with_site(site);
    }
    if let Ok(kind) = std::env::var(ENV_CONTROLLER) {
        config = config.with_controller(ControllerKind::parse(&kind));
    }
    if let Ok(insecure) = std::env::var(ENV_INSECURE) {
        let accept = !matches!(insecure.trim(), "0" | "false" | "no");
        config = config.with_accept_invalid_certs(accept);
    }
    Ok(UnifiClient::new(config))
}

/// Where a UniFi resource gets its [`UnifiClient`].
///
/// - [`Ready`](Self::Ready): a client built up front (e.g. from [`client_from_env`]).
/// - [`Vault`](Self::Vault): credentials are read from the controller's unlocked
///   vault at apply time. The `api_key` field is tried first (key auth); if it is
///   absent, `username` / `password` are used (session login) — the same precedence
///   as [`client_from_env`]. `host` / `site` / controller flavour are non-secret
///   config carried inline (mirrors `infrazeug-keycloak`'s configurable-field vault
///   source). The native node runs on the controller, which already holds the
///   unlocked vault session, so the client is built lazily inside `observe` /
///   `create` and cached for the run.
#[derive(Clone)]
pub enum UnifiClientSource {
    Ready(Arc<UnifiClient>),
    Vault {
        /// Controller base URL (non-secret).
        host: Arc<str>,
        /// Site shortname (non-secret).
        site: Arc<str>,
        /// Controller flavour (non-secret).
        controller: ControllerKind,
        /// Accept self-signed certificates (non-secret).
        accept_invalid_certs: bool,
        /// Vault file (under `files/`) holding the credential fields.
        file: Arc<str>,
        /// Field tried first; when present, key auth (`X-API-KEY`) is used.
        api_key_field: Arc<str>,
        username_field: Arc<str>,
        password_field: Arc<str>,
        /// Built once per run, shared across the resources of one builder.
        cache: Arc<OnceCell<Arc<UnifiClient>>>,
    },
}

impl UnifiClientSource {
    /// A ready client (no vault read).
    pub fn ready(client: UnifiClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    /// Read credentials from `file` in the controller vault at apply time: an
    /// `api_key` field (key auth) if present, otherwise `username` / `password`
    /// (session login). `host` is non-secret config; the site defaults to `default`
    /// and the controller flavour to UniFi OS (override with
    /// [`with_site`](Self::with_site) / [`with_controller`](Self::with_controller)).
    pub fn vault(host: impl Into<String>, file: impl Into<String>) -> Self {
        Self::vault_fields(host, file, FIELD_USERNAME, FIELD_PASSWORD)
    }

    /// Like [`vault`](Self::vault) with explicit session-credential field names.
    /// The API-key field name stays `api_key` (change it with
    /// [`with_api_key_field`](Self::with_api_key_field)).
    pub fn vault_fields(
        host: impl Into<String>,
        file: impl Into<String>,
        username_field: impl Into<String>,
        password_field: impl Into<String>,
    ) -> Self {
        Self::Vault {
            host: Arc::from(host.into().trim_end_matches('/').to_string()),
            site: Arc::from(infrazeug_ext_unifi_api::DEFAULT_SITE.to_string()),
            controller: ControllerKind::UnifiOs,
            accept_invalid_certs: false,
            file: Arc::from(file.into()),
            api_key_field: Arc::from(FIELD_API_KEY),
            username_field: Arc::from(username_field.into()),
            password_field: Arc::from(password_field.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }

    /// Override the vault field checked for an API key (default `api_key`). When
    /// that field resolves, key auth is used in preference to username/password.
    pub fn with_api_key_field(mut self, field: impl Into<String>) -> Self {
        if let Self::Vault { api_key_field, .. } = &mut self {
            *api_key_field = Arc::from(field.into());
        }
        self
    }

    /// Target a non-default site (vault sources only; no-op on a ready client).
    pub fn with_site(mut self, site_name: impl Into<String>) -> Self {
        if let Self::Vault { site, .. } = &mut self {
            *site = Arc::from(site_name.into());
        }
        self
    }

    /// Select the controller flavour (vault sources only).
    pub fn with_controller(mut self, kind: ControllerKind) -> Self {
        if let Self::Vault { controller, .. } = &mut self {
            *controller = kind;
        }
        self
    }

    /// Toggle acceptance of self-signed certificates (vault sources only).
    ///
    /// Verification is on by default; pass `true` to ignore the controller's TLS
    /// certificate. No-op on a ready client (configure its [`UnifiConfig`] instead).
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

    /// Skip TLS certificate verification for a vault source (ignore the
    /// controller's self-signed certificate). Shorthand for
    /// `with_accept_invalid_certs(true)`.
    pub fn insecure(self) -> Self {
        self.with_accept_invalid_certs(true)
    }

    /// Resolve to a usable client, reading the vault on first use.
    ///
    /// Returns [`ResourceError::SecretsUnavailable`] when a vault-backed source is used
    /// without an unlocked controller vault (e.g. read-only preview); callers in the
    /// plan path treat that as "unknown" rather than a failure.
    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<UnifiClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault {
                host,
                site,
                controller,
                accept_invalid_certs,
                file,
                api_key_field,
                username_field,
                password_field,
                cache,
            } => cache
                .get_or_try_init(|| {
                    build_from_vault(
                        ctx,
                        host,
                        site,
                        *controller,
                        *accept_invalid_certs,
                        file,
                        api_key_field,
                        username_field,
                        password_field,
                    )
                })
                .await
                .cloned(),
        }
    }
}

impl From<UnifiClient> for UnifiClientSource {
    fn from(client: UnifiClient) -> Self {
        Self::ready(client)
    }
}

impl From<Arc<UnifiClient>> for UnifiClientSource {
    fn from(client: Arc<UnifiClient>) -> Self {
        Self::Ready(client)
    }
}

#[allow(clippy::too_many_arguments)]
async fn build_from_vault(
    ctx: &ResourceCtx,
    host: &str,
    site: &str,
    controller: ControllerKind,
    accept_invalid_certs: bool,
    file: &str,
    api_key_field: &str,
    username_field: &str,
    password_field: &str,
) -> ResourceResult<Arc<UnifiClient>> {
    // Prefer an API key when one is sealed in the vault; fall back to a session
    // username/password pair otherwise (same precedence as `client_from_env`).
    let credentials = match read_optional(ctx, file, api_key_field).await? {
        Some(key) => Credentials::api_key(key),
        None => {
            let username = read_trimmed(ctx, file, username_field).await?;
            let password = read_trimmed(ctx, file, password_field).await?;
            Credentials::user_pass(username, password)
        }
    };

    let unifi = UnifiConfig::new(host.to_string(), credentials)
        .with_site(site.to_string())
        .with_controller(controller)
        .with_accept_invalid_certs(accept_invalid_certs);
    Ok(Arc::new(UnifiClient::new(unifi)))
}

/// Read a required credential field, trimming the trailing newline that sealed-from-
/// file secrets often carry (it would otherwise corrupt the auth header).
async fn read_trimmed(ctx: &ResourceCtx, file: &str, field: &str) -> ResourceResult<String> {
    Ok(ctx
        .read_secret_string(file, field)
        .await?
        .trim()
        .to_string())
}

/// Read an optional field: a missing field yields `None`, but an unavailable vault
/// (read-only preview) propagates [`ResourceError::SecretsUnavailable`] so the plan
/// path reports the resource as unknown rather than silently mis-authenticating.
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
        let client = UnifiClient::new(UnifiConfig::new(
            "https://unifi.local",
            Credentials::api_key("k"),
        ));
        let source = UnifiClientSource::ready(client);
        assert!(source.client(&ctx_without_secrets()).await.is_ok());
    }

    #[tokio::test]
    async fn vault_source_without_vault_is_secrets_unavailable() {
        let source = UnifiClientSource::vault("https://unifi.local", "cloud/unifi.vault");
        assert!(matches!(
            source.client(&ctx_without_secrets()).await,
            Err(ResourceError::SecretsUnavailable)
        ));
    }

    #[test]
    fn vault_api_key_field_defaults_and_overrides() {
        match UnifiClientSource::vault("https://unifi.local", "f") {
            UnifiClientSource::Vault { api_key_field, .. } => {
                assert_eq!(&*api_key_field, "api_key")
            }
            _ => panic!("expected vault source"),
        }
        match UnifiClientSource::vault("https://unifi.local", "f").with_api_key_field("token") {
            UnifiClientSource::Vault { api_key_field, .. } => assert_eq!(&*api_key_field, "token"),
            _ => panic!("expected vault source"),
        }
    }

    #[test]
    fn vault_default_verifies_tls() {
        match UnifiClientSource::vault("https://unifi.local", "f") {
            UnifiClientSource::Vault {
                accept_invalid_certs,
                ..
            } => assert!(!accept_invalid_certs),
            _ => panic!("expected vault source"),
        }
    }

    #[test]
    fn vault_setters_apply() {
        let source = UnifiClientSource::vault("https://unifi.local/", "f")
            .with_site("branch")
            .with_controller(ControllerKind::Legacy)
            .insecure();
        match source {
            UnifiClientSource::Vault {
                host,
                site,
                controller,
                accept_invalid_certs,
                ..
            } => {
                assert_eq!(&*host, "https://unifi.local");
                assert_eq!(&*site, "branch");
                assert_eq!(controller, ControllerKind::Legacy);
                assert!(accept_invalid_certs);
            }
            _ => panic!("expected vault source"),
        }
    }
}
