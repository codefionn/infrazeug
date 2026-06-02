//! Workers KV namespace management (`/accounts/{account_id}/storage/kv/namespaces`).

use crate::client::CloudflareClient;
use crate::error::{CloudflareError, Result};
use crate::types::ListQuery;
use serde::{Deserialize, Serialize};

/// A Workers KV namespace.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct KvNamespace {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_url_encoding: Option<bool>,
}

/// Body for `POST /accounts/{account_id}/storage/kv/namespaces`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct KvNamespaceCreate {
    pub title: String,
}

impl CloudflareClient {
    /// `GET /accounts/{account_id}/storage/kv/namespaces` — list namespaces (all pages).
    pub async fn kv_namespaces(
        &self,
        account_id: &str,
        query: &ListQuery,
    ) -> Result<Vec<KvNamespace>> {
        let path = format!(
            "/accounts/{}/storage/kv/namespaces",
            self.encode_path(account_id)
        );
        self.get_all(&path, query.clone()).await
    }

    /// `GET /accounts/{account_id}/storage/kv/namespaces/{id}` — fetch one namespace.
    pub async fn kv_namespace(&self, account_id: &str, namespace_id: &str) -> Result<KvNamespace> {
        let path = format!(
            "/accounts/{}/storage/kv/namespaces/{}",
            self.encode_path(account_id),
            self.encode_path(namespace_id)
        );
        let (namespace, _) = self.get(&path, &ListQuery::default()).await?;
        Ok(namespace)
    }

    /// `POST /accounts/{account_id}/storage/kv/namespaces` — create a namespace.
    pub async fn create_kv_namespace(
        &self,
        account_id: &str,
        body: &KvNamespaceCreate,
    ) -> Result<KvNamespace> {
        let path = format!(
            "/accounts/{}/storage/kv/namespaces",
            self.encode_path(account_id)
        );
        self.post_json(&path, body).await
    }

    /// `DELETE /accounts/{account_id}/storage/kv/namespaces/{id}` — delete a namespace.
    pub async fn delete_kv_namespace(&self, account_id: &str, namespace_id: &str) -> Result<()> {
        let path = format!(
            "/accounts/{}/storage/kv/namespaces/{}",
            self.encode_path(account_id),
            self.encode_path(namespace_id)
        );
        self.delete(&path).await
    }

    /// Find a namespace by title (exact match).
    pub async fn kv_namespace_by_title(
        &self,
        account_id: &str,
        title: &str,
    ) -> Result<Option<KvNamespace>> {
        let namespaces = self
            .kv_namespaces(
                account_id,
                &ListQuery {
                    per_page: Some(100),
                    ..Default::default()
                },
            )
            .await?;
        Ok(namespaces.into_iter().find(|ns| ns.title == title))
    }

    /// Like [`kv_namespace_by_title`](Self::kv_namespace_by_title) but errors when absent.
    pub async fn require_kv_namespace_by_title(
        &self,
        account_id: &str,
        title: &str,
    ) -> Result<KvNamespace> {
        self.kv_namespace_by_title(account_id, title)
            .await?
            .ok_or_else(|| CloudflareError::Api {
                status: 404,
                codes: vec![],
                message: format!("kv namespace not found: {title}"),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kv_namespace_deserializes() {
        let body = r#"{
            "id": "0f2ac74b498b48028cb68387c421e279",
            "title": "My Namespace",
            "supports_url_encoding": true
        }"#;
        let ns: KvNamespace = serde_json::from_str(body).unwrap();
        assert_eq!(ns.id, "0f2ac74b498b48028cb68387c421e279");
        assert_eq!(ns.title, "My Namespace");
        assert_eq!(ns.supports_url_encoding, Some(true));
    }

    #[test]
    fn kv_namespace_create_serializes() {
        let body = KvNamespaceCreate {
            title: "cache".into(),
        };
        let json = serde_json::to_string(&body).unwrap();
        assert_eq!(json, r#"{"title":"cache"}"#);
    }
}
