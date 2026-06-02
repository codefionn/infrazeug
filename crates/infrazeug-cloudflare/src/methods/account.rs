//! Shared account id resolution for account-scoped Cloudflare resources.

use crate::client::CloudflareClientSource;
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};

/// Resolve a Cloudflare account id from explicit id, config default, or account name.
pub(crate) async fn resolve_account_id(
    source: &CloudflareClientSource,
    ctx: &ResourceCtx,
    account_id: &Option<String>,
    account_name: &Option<String>,
) -> ResourceResult<String> {
    let client = source.client(ctx).await?;
    client
        .resolve_account_id(account_id.as_deref(), account_name.as_deref())
        .await
        .map_err(ResourceError::provider)
}
