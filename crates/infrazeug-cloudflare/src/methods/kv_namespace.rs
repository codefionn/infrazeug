//! Ensure a Workers KV namespace exists.

use crate::client::CloudflareClientSource;
use crate::methods::account::resolve_account_id;
use async_trait::async_trait;
use infrazeug_ext_cloudflare_api::kv_namespace::KvNamespaceCreate;
use infrazeug_resource::{EnsureResource, Resource, ResourceCtx, ResourceError, ResourceResult};
use serde::{Deserialize, Serialize};

pub const ENSURE_KV_NAMESPACE: &str = "cloudflare.ensure_kv_namespace";

/// Tier-1 method: ensure a Workers KV namespace exists.
pub type EnsureKvNamespace = EnsureResource<KvNamespaceResource>;

/// Construct the registrable [`EnsureKvNamespace`] method for a client source.
pub fn ensure_kv_namespace(source: CloudflareClientSource) -> EnsureKvNamespace {
    EnsureResource::new(KvNamespaceResource::new(source))
}

/// Desired KV namespace. Natural key: account + `title`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureKvNamespaceInput {
    /// Account id (32-char hex). Provide this, [`account_name`](Self::account_name), or set `CLOUDFLARE_ACCOUNT_ID`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// Account display name (resolved via `GET /accounts?name=…`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_name: Option<String>,
    /// Human-readable namespace title (immutable after create).
    pub title: String,
}

/// Observed KV namespace — managed fields from the API.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct EnsureKvNamespaceOutput {
    pub account_id: String,
    pub namespace_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_url_encoding: Option<bool>,
}

#[derive(Clone)]
pub struct KvNamespaceResource {
    source: CloudflareClientSource,
}

impl KvNamespaceResource {
    pub fn new(source: CloudflareClientSource) -> Self {
        Self { source }
    }
}

fn to_output(
    account_id: &str,
    namespace: infrazeug_ext_cloudflare_api::kv_namespace::KvNamespace,
) -> EnsureKvNamespaceOutput {
    EnsureKvNamespaceOutput {
        account_id: account_id.to_string(),
        namespace_id: namespace.id,
        title: namespace.title,
        supports_url_encoding: namespace.supports_url_encoding,
    }
}

#[async_trait]
impl Resource for KvNamespaceResource {
    type Spec = EnsureKvNamespaceInput;
    type State = EnsureKvNamespaceOutput;

    fn kind(&self) -> &'static str {
        ENSURE_KV_NAMESPACE
    }

    async fn observe(
        &self,
        ctx: &ResourceCtx,
        spec: &Self::Spec,
    ) -> ResourceResult<Option<Self::State>> {
        let account_id =
            resolve_account_id(&self.source, ctx, &spec.account_id, &spec.account_name).await?;
        let client = self.source.client(ctx).await?;
        Ok(client
            .kv_namespace_by_title(&account_id, &spec.title)
            .await
            .map_err(ResourceError::provider)?
            .map(|ns| to_output(&account_id, ns)))
    }

    async fn create(&self, ctx: &ResourceCtx, spec: &Self::Spec) -> ResourceResult<Self::State> {
        let account_id =
            resolve_account_id(&self.source, ctx, &spec.account_id, &spec.account_name).await?;
        let client = self.source.client(ctx).await?;
        let created = client
            .create_kv_namespace(
                &account_id,
                &KvNamespaceCreate {
                    title: spec.title.clone(),
                },
            )
            .await
            .map_err(ResourceError::provider)?;
        Ok(to_output(&account_id, created))
    }

    // Title is the namespace identity; nothing mutable to reconcile.
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_resource::Drift;

    fn resource() -> KvNamespaceResource {
        KvNamespaceResource::new(CloudflareClientSource::vault("cloud/cloudflare.vault"))
    }

    #[test]
    fn matching_spec_is_in_sync() {
        let spec = EnsureKvNamespaceInput {
            account_id: Some("acc123".into()),
            title: "cache".into(),
            ..Default::default()
        };
        let current = EnsureKvNamespaceOutput {
            account_id: "acc123".into(),
            namespace_id: "ns456".into(),
            title: "cache".into(),
            supports_url_encoding: Some(true),
        };
        assert_eq!(resource().diff(&spec, &current), Drift::InSync);
    }
}
