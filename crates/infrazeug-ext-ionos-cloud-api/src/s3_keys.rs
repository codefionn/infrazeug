//! User Object Storage key management (`/um/users/{id}/s3keys`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ListQuery};

pub use crate::um_types::{S3Key, S3KeyProperties, S3KeyUpdate, S3SsoUrl};

fn s3_key_path(client: &IonosClient, user_id: &str, key_id: Option<&str>) -> String {
    let mut path = format!("/um/users/{}/s3keys", client.encode_path(user_id));
    if let Some(key_id) = key_id {
        path.push('/');
        path.push_str(&client.encode_path(key_id));
    }
    path
}

impl IonosClient {
    /// `GET /um/users/{id}/s3keys` — list Object Storage keys for a user.
    pub async fn user_s3_keys(
        &self,
        user_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<S3Key>> {
        self.get(&s3_key_path(self, user_id, None), query).await
    }

    /// `POST /um/users/{id}/s3keys` — create an Object Storage key (max 5 per user).
    pub async fn create_user_s3_key(&self, user_id: &str, query: &ListQuery) -> Result<S3Key> {
        self.post_json(
            &s3_key_path(self, user_id, None),
            &serde_json::json!({}),
            query,
        )
        .await
    }

    /// `GET /um/users/{id}/s3keys/{keyId}` — retrieve one Object Storage key.
    pub async fn user_s3_key(
        &self,
        user_id: &str,
        key_id: &str,
        query: &ListQuery,
    ) -> Result<S3Key> {
        self.get(&s3_key_path(self, user_id, Some(key_id)), query)
            .await
    }

    /// `PUT /um/users/{id}/s3keys/{keyId}` — enable or disable a key.
    pub async fn update_user_s3_key(
        &self,
        user_id: &str,
        key_id: &str,
        body: &S3KeyUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<S3Key> {
        self.put_json(&s3_key_path(self, user_id, Some(key_id)), body, query, etag)
            .await
    }

    /// `DELETE /um/users/{id}/s3keys/{keyId}` — delete an Object Storage key.
    pub async fn delete_user_s3_key(
        &self,
        user_id: &str,
        key_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(&s3_key_path(self, user_id, Some(key_id)), query, etag)
            .await
    }

    /// `GET /um/users/{id}/s3ssourl` — retrieve Object Storage SSO URL.
    pub async fn user_s3_sso_url(&self, user_id: &str) -> Result<S3SsoUrl> {
        self.get(
            &format!("/um/users/{}/s3ssourl", self.encode_path(user_id)),
            &ListQuery::default(),
        )
        .await
    }
}
