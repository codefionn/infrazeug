//! Controller-side NetBird client construction.

use infrazeug_ext_netbird_api::{Auth, NetBirdClient, NetBirdConfig};
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

const ENV_TOKEN: &str = "NETBIRD_TOKEN";
const ENV_OAUTH_TOKEN: &str = "NETBIRD_OAUTH_TOKEN";
const ENV_HOST: &str = "NETBIRD_HOST";
const FIELD_TOKEN: &str = "token";
const FIELD_OAUTH_TOKEN: &str = "oauth_token";
const FIELD_HOST: &str = "host";

/// Build a client from `NETBIRD_TOKEN` or `NETBIRD_OAUTH_TOKEN`, plus an
/// optional `NETBIRD_HOST`.
pub fn client_from_env() -> anyhow::Result<NetBirdClient> {
    let auth = if let Ok(token) = std::env::var(ENV_TOKEN) {
        Auth::personal_access_token(token)
    } else {
        let token = std::env::var(ENV_OAUTH_TOKEN)
            .map_err(|_| anyhow::anyhow!("set {ENV_TOKEN} or {ENV_OAUTH_TOKEN}"))?;
        Auth::oauth_token(token)
    };
    let mut config = NetBirdConfig::new(auth);
    if let Ok(host) = std::env::var(ENV_HOST) {
        if !host.trim().is_empty() {
            config = config.with_host(host);
        }
    }
    Ok(NetBirdClient::new(config))
}

/// Where NetBird resources get their client.
#[derive(Clone)]
pub enum NetBirdClientSource {
    Ready(Arc<NetBirdClient>),
    /// Credentials in a regular, user-managed controller vault file.
    Vault {
        file: Arc<str>,
        host: Option<Arc<str>>,
        cache: Arc<OnceCell<Arc<NetBirdClient>>>,
    },
    /// Credentials generated and maintained in `files/mutable/{file}`.
    MutableVault {
        file: Arc<str>,
        host: Option<Arc<str>>,
        cache: Arc<OnceCell<Arc<NetBirdClient>>>,
    },
}

impl NetBirdClientSource {
    pub fn ready(client: NetBirdClient) -> Self {
        Self::Ready(Arc::new(client))
    }

    /// Read `token` (preferred) or `oauth_token` from a controller vault file at
    /// apply time. A `host` field is optional.
    pub fn vault(file: impl Into<String>) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            host: None,
            cache: Arc::new(OnceCell::new()),
        }
    }

    /// Read `token` (preferred) or `oauth_token` from a generated mutable vault
    /// file at apply time. The optional `host` field supports self-hosted NetBird.
    ///
    /// `file` is relative to `files/mutable/`, so
    /// `mutable_vault("netbird/credentials.vault")` reads
    /// `files/mutable/netbird/credentials.vault`.
    pub fn mutable_vault(file: impl Into<String>) -> Self {
        Self::MutableVault {
            file: Arc::from(file.into()),
            host: None,
            cache: Arc::new(OnceCell::new()),
        }
    }

    /// Use this Management API host while still reading the credential from vault.
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        match &mut self {
            Self::Vault {
                host: configured, ..
            }
            | Self::MutableVault {
                host: configured, ..
            } => *configured = Some(Arc::from(host.into())),
            Self::Ready(_) => {}
        }
        self
    }

    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<NetBirdClient>> {
        match self {
            Self::Ready(client) => Ok(Arc::clone(client)),
            Self::Vault { file, host, cache } => cache
                .get_or_try_init(|| build_from_vault(ctx, file, host.as_deref(), VaultKind::Static))
                .await
                .cloned(),
            Self::MutableVault { file, host, cache } => cache
                .get_or_try_init(|| {
                    build_from_vault(ctx, file, host.as_deref(), VaultKind::Mutable)
                })
                .await
                .cloned(),
        }
    }
}

impl From<NetBirdClient> for NetBirdClientSource {
    fn from(client: NetBirdClient) -> Self {
        Self::ready(client)
    }
}

async fn build_from_vault(
    ctx: &ResourceCtx,
    file: &str,
    host_override: Option<&str>,
    kind: VaultKind,
) -> ResourceResult<Arc<NetBirdClient>> {
    let auth = match read_vault_field(ctx, kind, file, FIELD_TOKEN).await {
        Ok(token) => Auth::personal_access_token(token.trim()),
        Err(ResourceError::SecretsUnavailable) => return Err(ResourceError::SecretsUnavailable),
        Err(ResourceError::InputsUnavailable) => return Err(ResourceError::InputsUnavailable),
        Err(ResourceError::Provider(_)) => Auth::oauth_token(
            read_vault_field(ctx, kind, file, FIELD_OAUTH_TOKEN)
                .await?
                .trim(),
        ),
        Err(error) => return Err(error),
    };
    let mut config = NetBirdConfig::new(auth);
    if let Some(host) = host_override {
        config = config.with_host(host);
    } else {
        match read_vault_field(ctx, kind, file, FIELD_HOST).await {
            Ok(host) if !host.trim().is_empty() => config = config.with_host(host),
            Ok(_) | Err(ResourceError::Provider(_)) => {}
            Err(ResourceError::SecretsUnavailable) => {
                return Err(ResourceError::SecretsUnavailable)
            }
            Err(ResourceError::InputsUnavailable) => return Err(ResourceError::InputsUnavailable),
            Err(error) => return Err(error),
        }
    }
    Ok(Arc::new(NetBirdClient::new(config)))
}

#[derive(Clone, Copy)]
enum VaultKind {
    Static,
    Mutable,
}

async fn read_vault_field(
    ctx: &ResourceCtx,
    kind: VaultKind,
    file: &str,
    field: &str,
) -> ResourceResult<String> {
    match kind {
        VaultKind::Static => ctx.read_secret_string(file, field).await,
        VaultKind::Mutable => ctx.read_mutable_secret_string(file, field).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infrazeug_native::{NativeError, NodeCtx, SecretSource};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    #[tokio::test]
    async fn vault_source_is_unavailable_without_a_vault() {
        let ctx = ResourceCtx::from(&NodeCtx::new(Uuid::nil(), Uuid::nil()));
        assert!(matches!(
            NetBirdClientSource::vault("netbird.vault")
                .client(&ctx)
                .await,
            Err(ResourceError::SecretsUnavailable)
        ));
    }

    struct MutableSecrets(BTreeMap<&'static str, &'static str>);

    #[async_trait]
    impl SecretSource for MutableSecrets {
        async fn read_field(&self, _file: &str, _field: &str) -> infrazeug_native::Result<Vec<u8>> {
            Err(NativeError::other(
                "static vault is deliberately unavailable",
            ))
        }

        async fn read_mutable_field(
            &self,
            file: &str,
            field: &str,
        ) -> infrazeug_native::Result<Vec<u8>> {
            assert_eq!(file, "netbird/credentials.vault");
            self.0
                .get(field)
                .map(|value| value.as_bytes().to_vec())
                .ok_or_else(|| NativeError::other(format!("missing {field}")))
        }
    }

    fn mutable_ctx(fields: &[(&'static str, &'static str)]) -> ResourceCtx {
        let ctx = NodeCtx::new(Uuid::nil(), Uuid::nil()).with_secrets(Some(
            Arc::new(MutableSecrets(fields.iter().copied().collect())) as Arc<dyn SecretSource>,
        ));
        ResourceCtx::from(&ctx)
    }

    #[tokio::test]
    async fn mutable_vault_source_reads_pat_and_host() {
        let ctx = mutable_ctx(&[
            (FIELD_TOKEN, " mutable-pat "),
            (FIELD_HOST, "https://netbird.internal///"),
        ]);
        let client = NetBirdClientSource::mutable_vault("netbird/credentials.vault")
            .client(&ctx)
            .await
            .unwrap();

        assert_eq!(client.config().host, "https://netbird.internal");
        assert!(matches!(
            client.config().auth,
            Auth::PersonalAccessToken(ref token) if token == "mutable-pat"
        ));
    }

    #[tokio::test]
    async fn mutable_vault_source_falls_back_to_oauth_token() {
        let ctx = mutable_ctx(&[(FIELD_OAUTH_TOKEN, "mutable-oauth")]);
        let client = NetBirdClientSource::mutable_vault("netbird/credentials.vault")
            .with_host("https://configured.example")
            .client(&ctx)
            .await
            .unwrap();

        assert_eq!(client.config().host, "https://configured.example");
        assert!(matches!(
            client.config().auth,
            Auth::OAuthToken(ref token) if token == "mutable-oauth"
        ));
    }
}
