//! Controller-side OVH client construction (environment or vault credentials).

use infrazeug_ext_ovh_api::{Credentials, OvhClient, OvhEndpoint};
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// Vault field names read by [`OvhClientSource::Vault`].
const FIELD_APPLICATION_KEY: &str = "application_key";
const FIELD_APPLICATION_SECRET: &str = "application_secret";
const FIELD_CONSUMER_KEY: &str = "consumer_key";
const FIELD_ENDPOINT: &str = "endpoint";

/// Build an [`OvhClient`] from standard OVH environment variables.
///
/// Required: `OVH_APPLICATION_KEY`, `OVH_APPLICATION_SECRET`, `OVH_CONSUMER_KEY`.
/// Optional: `OVH_ENDPOINT` (`eu` default, or `us`, `ca`).
pub fn client_from_env() -> anyhow::Result<OvhClient> {
    let endpoint =
        endpoint_from_name(&std::env::var("OVH_ENDPOINT").unwrap_or_else(|_| "eu".into()));
    Ok(OvhClient::new(
        endpoint,
        std::env::var("OVH_APPLICATION_KEY")
            .map_err(|_| anyhow::anyhow!("OVH_APPLICATION_KEY is not set"))?,
        std::env::var("OVH_APPLICATION_SECRET")
            .map_err(|_| anyhow::anyhow!("OVH_APPLICATION_SECRET is not set"))?,
        std::env::var("OVH_CONSUMER_KEY")
            .map_err(|_| anyhow::anyhow!("OVH_CONSUMER_KEY is not set"))?,
    ))
}

pub(crate) fn endpoint_from_name(name: &str) -> OvhEndpoint {
    match name.trim().to_ascii_lowercase().as_str() {
        "us" => OvhEndpoint::OvhUs,
        "ca" => OvhEndpoint::OvhCa,
        _ => OvhEndpoint::OvhEu,
    }
}

/// Where an OVH resource gets its [`OvhClient`].
///
/// - [`Ready`](Self::Ready): a client built up front (e.g. from [`client_from_env`]).
/// - [`Vault`](Self::Vault): classic AK/AS/CK credentials read from the controller's
///   unlocked vault at apply time, so no `OVH_*` secrets need to live in the environment.
///   The native node runs on the controller, which already holds the unlocked vault
///   session, so the client is built lazily inside `observe`/`create` and cached.
/// - [`VaultOAuth2`](Self::VaultOAuth2): like [`Vault`](Self::Vault) but OAuth2
///   service-account credentials, with configurable field names so existing vault fields
///   can be reused without re-sealing.
#[derive(Clone)]
pub enum OvhClientSource {
    Ready(Arc<OvhClient>),
    Vault {
        /// Vault file (under `files/`) holding the credential fields.
        file: Arc<str>,
        /// Built once per run, shared across the resources of one builder.
        cache: Arc<OnceCell<Arc<OvhClient>>>,
    },
    VaultOAuth2 {
        /// Vault file (under `files/`) holding the OAuth2 credential fields.
        file: Arc<str>,
        /// Field holding the OAuth2 `client_id`.
        client_id_field: Arc<str>,
        /// Field holding the OAuth2 `client_secret`.
        client_secret_field: Arc<str>,
        /// Built once per run, shared across the resources of one builder.
        cache: Arc<OnceCell<Arc<OvhClient>>>,
    },
}

impl OvhClientSource {
    /// A ready client (no vault read).
    pub fn ready(client: OvhClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    /// Credentials read from `file` in the controller vault at apply time.
    pub fn vault(file: impl Into<String>) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }

    /// OAuth2 credentials read from `file` (fields `client_id_field`/`client_secret_field`)
    /// in the controller vault at apply time. The OVH API endpoint defaults to EU.
    pub fn vault_oauth2(
        file: impl Into<String>,
        client_id_field: impl Into<String>,
        client_secret_field: impl Into<String>,
    ) -> Self {
        Self::VaultOAuth2 {
            file: Arc::from(file.into()),
            client_id_field: Arc::from(client_id_field.into()),
            client_secret_field: Arc::from(client_secret_field.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }

    /// Resolve to a usable client, reading the vault on first use.
    ///
    /// Returns [`ResourceError::SecretsUnavailable`] when a vault-backed source is used
    /// without an unlocked controller vault (e.g. read-only preview); callers in the
    /// plan path treat that as "unknown" rather than a failure.
    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<OvhClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault { file, cache } => cache
                .get_or_try_init(|| build_from_vault(ctx, file))
                .await
                .cloned(),
            Self::VaultOAuth2 {
                file,
                client_id_field,
                client_secret_field,
                cache,
            } => cache
                .get_or_try_init(|| {
                    build_from_vault_oauth2(ctx, file, client_id_field, client_secret_field)
                })
                .await
                .cloned(),
        }
    }
}

impl From<Arc<OvhClient>> for OvhClientSource {
    fn from(client: Arc<OvhClient>) -> Self {
        Self::Ready(client)
    }
}

impl From<OvhClient> for OvhClientSource {
    fn from(client: OvhClient) -> Self {
        Self::ready(client)
    }
}

/// Read a credential field, trimming surrounding whitespace. Secrets sealed from a file or
/// `echo` often carry a trailing newline; shell consumers strip it via `$(...)`, but these
/// values go straight into HTTP auth headers, where a stray `\n` yields `invalid_client`.
async fn read_secret_trimmed(ctx: &ResourceCtx, file: &str, field: &str) -> ResourceResult<String> {
    Ok(ctx
        .read_secret_string(file, field)
        .await?
        .trim()
        .to_string())
}

async fn build_from_vault(ctx: &ResourceCtx, file: &str) -> ResourceResult<Arc<OvhClient>> {
    let application_key = read_secret_trimmed(ctx, file, FIELD_APPLICATION_KEY).await?;
    let application_secret = read_secret_trimmed(ctx, file, FIELD_APPLICATION_SECRET).await?;
    let consumer_key = read_secret_trimmed(ctx, file, FIELD_CONSUMER_KEY).await?;
    // Endpoint is optional metadata, not a secret; default to EU when absent.
    let endpoint = match ctx.read_secret_string(file, FIELD_ENDPOINT).await {
        Ok(name) => endpoint_from_name(&name),
        Err(ResourceError::SecretsUnavailable) => return Err(ResourceError::SecretsUnavailable),
        Err(_) => OvhEndpoint::OvhEu,
    };
    Ok(Arc::new(OvhClient::from_credentials(
        endpoint,
        Credentials::new(application_key, application_secret, consumer_key),
    )))
}

async fn build_from_vault_oauth2(
    ctx: &ResourceCtx,
    file: &str,
    client_id_field: &str,
    client_secret_field: &str,
) -> ResourceResult<Arc<OvhClient>> {
    let client_id = read_secret_trimmed(ctx, file, client_id_field).await?;
    let client_secret = read_secret_trimmed(ctx, file, client_secret_field).await?;
    // OAuth2 service accounts are region-scoped; EU covers this deployment's project.
    Ok(Arc::new(OvhClient::oauth2(
        OvhEndpoint::OvhEu,
        client_id,
        client_secret,
    )))
}
