use infrazeug_ext_authentik_api::AuthentikClient;
use infrazeug_resource::{ResourceCtx, ResourceResult};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// An already constructed client or credentials in the controller vault.
#[derive(Clone)]
pub enum AuthentikClientSource {
    Ready(Arc<AuthentikClient>),
    Vault {
        file: Arc<str>,
        cache: Arc<OnceCell<Arc<AuthentikClient>>>,
    },
}

impl AuthentikClientSource {
    pub fn ready(client: AuthentikClient) -> Self {
        Self::Ready(Arc::new(client))
    }
    /// Read `base_url` and `token` fields from a vault file during apply.
    pub fn vault(file: impl Into<String>) -> Self {
        Self::Vault {
            file: Arc::from(file.into()),
            cache: Arc::new(OnceCell::new()),
        }
    }
    pub async fn client(&self, ctx: &ResourceCtx) -> ResourceResult<Arc<AuthentikClient>> {
        match self {
            Self::Ready(client) => Ok(client.clone()),
            Self::Vault { file, cache } => cache
                .get_or_try_init(|| async {
                    let base_url = ctx
                        .read_secret_string(file, "base_url")
                        .await?
                        .trim()
                        .to_owned();
                    let token = ctx
                        .read_secret_string(file, "token")
                        .await?
                        .trim()
                        .to_owned();
                    let client = AuthentikClient::new(base_url, token)
                        .map_err(infrazeug_resource::ResourceError::provider)?;
                    Ok(Arc::new(client))
                })
                .await
                .cloned(),
        }
    }
}

impl From<AuthentikClient> for AuthentikClientSource {
    fn from(client: AuthentikClient) -> Self {
        Self::ready(client)
    }
}

/// Build a client from `AUTHENTIK_URL` and `AUTHENTIK_TOKEN`.
pub fn client_from_env() -> anyhow::Result<AuthentikClient> {
    let url = std::env::var("AUTHENTIK_URL")?;
    let token = std::env::var("AUTHENTIK_TOKEN")?;
    Ok(AuthentikClient::new(url, token)?)
}
