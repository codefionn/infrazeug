//! Shared zone id resolution for Cloudflare resources.

use crate::client::CloudflareClientSource;
use infrazeug_resource::{ResourceCtx, ResourceError, ResourceResult};

/// Resolve a zone id from explicit id or DNS name.
pub(crate) async fn resolve_zone_id(
    source: &CloudflareClientSource,
    ctx: &ResourceCtx,
    zone_id: &Option<String>,
    zone_name: &Option<String>,
) -> ResourceResult<String> {
    if let Some(id) = zone_id {
        return Ok(id.clone());
    }
    let name = zone_name
        .as_deref()
        .ok_or_else(|| ResourceError::provider("zone_id or zone_name is required"))?;
    let client = source.client(ctx).await?;
    client
        .zone_id_by_name(name)
        .await
        .map_err(ResourceError::provider)
}
