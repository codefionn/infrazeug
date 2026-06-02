//! Controller-side Keycloak client construction (environment or vault credentials).

use infrazeug_ext_keycloak_admin::{GrantType, KeycloakClient, KeycloakConfig};
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// Vault field names read by [`KeycloakClientSource::Vault`].
const FIELD_BASE_URL: &str = "base_url";
const FIELD_REALM: &str = "realm";
const FIELD_CLIENT_ID: &str = "client_id";
const FIELD_CLIENT_SECRET: &str = "client_secret";

/// Token realm and admin client id used when the vault file / environment omit them.
const DEFAULT_TOKEN_REALM: &str = "master";
const DEFAULT_CLIENT_ID: &str = "admin-cli";

/// Build a [`KeycloakClient`] from standard environment variables.
///
/// Required: `KEYCLOAK_URL`. Optional: `KEYCLOAK_REALM` (token realm, `master`
/// default), `KEYCLOAK_CLIENT_ID` (`admin-cli` default).
///
/// Grant selection: when `KEYCLOAK_CLIENT_SECRET` is set, a `client_credentials`
/// (service-account) grant is used; otherwise `KEYCLOAK_USER` + `KEYCLOAK_PASSWORD`
/// select a `password` grant.
pub fn client_from_env() -> anyhow::Result<KeycloakClient> {
    let base_url =
        std::env::var("KEYCLOAK_URL").map_err(|_| anyhow::anyhow!("KEYCLOAK_URL is not set"))?;
    let realm = std::env::var("KEYCLOAK_REALM").unwrap_or_else(|_| DEFAULT_TOKEN_REALM.into());
    let client_id =
        std::env::var("KEYCLOAK_CLIENT_ID").unwrap_or_else(|_| DEFAULT_CLIENT_ID.into());

    let grant = if let Ok(client_secret) = std::env::var("KEYCLOAK_CLIENT_SECRET") {
        GrantType::ClientCredentials {
            client_id,
            client_secret,
        }
    } else if let (Ok(username), Ok(password)) = (
        std::env::var("KEYCLOAK_USER"),
        std::env::var("KEYCLOAK_PASSWORD"),
    ) {
        GrantType::Password {
            client_id,
            client_secret: None,
            username,
            password,
        }
    } else {
        anyhow::bail!(
            "set KEYCLOAK_CLIENT_SECRET (client_credentials) or KEYCLOAK_USER + KEYCLOAK_PASSWORD (password grant)"
        );
    };

    Ok(KeycloakClient::new(KeycloakConfig::new(
        base_url, realm, grant,
    )))
}

/// Where a Keycloak resource gets its [`KeycloakClient`].
///
/// - [`Ready`](Self::Ready): a client built up front (e.g. from [`client_from_env`]).
/// - [`Vault`](Self::Vault): `client_credentials` service-account credentials read from
///   the controller's unlocked vault at apply time, so no `KEYCLOAK_*` secrets need to
///   live in the environment. The native node runs on the controller, which already
///   holds the unlocked vault session, so the client is built lazily inside
///   `observe`/`create` and cached for the run.
/// - [`VaultPassword`](Self::VaultPassword): a `password` (direct-access) grant whose secrets —
///   both the username and the password — are read from *existing* vault fields at apply time,
///   so a dedicated API admin (`vault_keycloak_api_admin_user` / `vault_keycloak_api_admin_password`)
///   can be reused without sealing a new service-account client. `base_url`/`token_realm`/
///   `client_id` are non-secret config carried inline (mirrors `infrazeug-ovh`'s
///   configurable-field vault sources).
#[derive(Clone)]
pub enum KeycloakClientSource {
    Ready(Arc<KeycloakClient>),
    Vault {
        /// Vault file (under `files/`) holding the credential fields.
        file: Arc<str>,
        /// Built once per run, shared across the resources of one builder.
        cache: Arc<OnceCell<Arc<KeycloakClient>>>,
    },
    VaultPassword {
        base_url: Arc<str>,
        token_realm: Arc<str>,
        client_id: Arc<str>,
        /// Vault file (under `files/`) holding the username and password fields.
        file: Arc<str>,
        username_field: Arc<str>,
        password_field: Arc<str>,
        /// Built once per run, shared across the resources of one builder.
        cache: Arc<OnceCell<Arc<KeycloakClient>>>,
    },
}

impl KeycloakClientSource {
    /// A ready client (no vault read).
    pub fn ready(client: KeycloakClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    /// `client_credentials` credentials read from `file` in the controller vault at
    /// apply time.
    ///
    /// Fields: `base_url`, `client_secret` (required); `realm` (token realm, `master`
    /// default) and `client_id` (`admin-cli` default) are optional. `base_url`/`realm`
    /// are not secrets but live alongside the secret for convenience.
    pub fn vault(file: impl Into<String>) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }

    /// A `password` (direct-access) grant whose username and password are read from
    /// `username_field` / `password_field` in `file` at apply time. `base_url`/`token_realm`/
    /// `client_id` are non-secret config (e.g. `https://id…`, `master`, `admin-cli`); both
    /// credentials live in the vault, so existing fields can be reused without re-sealing.
    #[allow(clippy::too_many_arguments)]
    pub fn vault_password(
        base_url: impl Into<String>,
        token_realm: impl Into<String>,
        client_id: impl Into<String>,
        file: impl Into<String>,
        username_field: impl Into<String>,
        password_field: impl Into<String>,
    ) -> Self {
        Self::VaultPassword {
            base_url: Arc::from(base_url.into()),
            token_realm: Arc::from(token_realm.into()),
            client_id: Arc::from(client_id.into()),
            file: Arc::from(file.into()),
            username_field: Arc::from(username_field.into()),
            password_field: Arc::from(password_field.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }

    /// Resolve to a usable client, reading the vault on first use.
    ///
    /// Returns [`ResourceError::SecretsUnavailable`] when a vault-backed source is used
    /// without an unlocked controller vault (e.g. read-only preview); callers in the
    /// plan path treat that as "unknown" rather than a failure.
    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<KeycloakClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault { file, cache } => cache
                .get_or_try_init(|| build_from_vault(ctx, file))
                .await
                .cloned(),
            Self::VaultPassword {
                base_url,
                token_realm,
                client_id,
                file,
                username_field,
                password_field,
                cache,
            } => cache
                .get_or_try_init(|| {
                    build_from_vault_password(
                        ctx,
                        base_url,
                        token_realm,
                        client_id,
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

impl From<KeycloakClient> for KeycloakClientSource {
    fn from(client: KeycloakClient) -> Self {
        Self::ready(client)
    }
}

impl From<Arc<KeycloakClient>> for KeycloakClientSource {
    fn from(client: Arc<KeycloakClient>) -> Self {
        Self::Ready(client)
    }
}

/// Read a credential field, trimming surrounding whitespace. Secrets sealed from a file
/// or `echo` often carry a trailing newline; these values go straight into HTTP auth
/// headers / URLs, where a stray `\n` breaks token acquisition.
async fn read_secret_trimmed(ctx: &ResourceCtx, file: &str, field: &str) -> ResourceResult<String> {
    Ok(ctx
        .read_secret_string(file, field)
        .await?
        .trim()
        .to_string())
}

/// Read an optional field: a missing field yields `None`, but an unavailable vault
/// (read-only preview) propagates [`ResourceError::SecretsUnavailable`] so the plan path
/// reports the resource as unknown rather than silently defaulting.
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

async fn build_from_vault(ctx: &ResourceCtx, file: &str) -> ResourceResult<Arc<KeycloakClient>> {
    let base_url = read_secret_trimmed(ctx, file, FIELD_BASE_URL).await?;
    let realm = read_optional(ctx, file, FIELD_REALM)
        .await?
        .unwrap_or_else(|| DEFAULT_TOKEN_REALM.into());
    let client_id = read_optional(ctx, file, FIELD_CLIENT_ID)
        .await?
        .unwrap_or_else(|| DEFAULT_CLIENT_ID.into());
    let client_secret = read_secret_trimmed(ctx, file, FIELD_CLIENT_SECRET).await?;

    Ok(Arc::new(KeycloakClient::new(KeycloakConfig::new(
        base_url,
        realm,
        GrantType::ClientCredentials {
            client_id,
            client_secret,
        },
    ))))
}

#[allow(clippy::too_many_arguments)]
async fn build_from_vault_password(
    ctx: &ResourceCtx,
    base_url: &str,
    token_realm: &str,
    client_id: &str,
    file: &str,
    username_field: &str,
    password_field: &str,
) -> ResourceResult<Arc<KeycloakClient>> {
    let username = read_secret_trimmed(ctx, file, username_field).await?;
    let password = read_secret_trimmed(ctx, file, password_field).await?;

    Ok(Arc::new(KeycloakClient::new(KeycloakConfig::new(
        base_url,
        token_realm,
        GrantType::Password {
            client_id: client_id.to_string(),
            client_secret: None,
            username,
            password,
        },
    ))))
}
