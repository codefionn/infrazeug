//! User management (`/um/users`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ListQuery};

use crate::um_types::{Group, UmResource};
pub use crate::um_types::{
    User, UserCreate, UserCreateProperties, UserMetadata, UserProperties, UserUpdate,
    UserUpdateProperties,
};

fn user_path(client: &IonosClient, user_id: Option<&str>) -> String {
    let mut path = "/um/users".to_string();
    if let Some(user_id) = user_id {
        path.push('/');
        path.push_str(&client.encode_path(user_id));
    }
    path
}

impl IonosClient {
    /// `GET /um/users` — list all users in the account.
    pub async fn users(&self, query: &ListQuery) -> Result<Collection<User>> {
        self.get(&user_path(self, None), query).await
    }

    /// `GET /um/users/{id}` — retrieve one user.
    pub async fn user(&self, user_id: &str, query: &ListQuery) -> Result<User> {
        self.get(&user_path(self, Some(user_id)), query).await
    }

    /// `POST /um/users` — create a user.
    pub async fn create_user(&self, body: &UserCreate, query: &ListQuery) -> Result<User> {
        self.post_json(&user_path(self, None), body, query).await
    }

    /// `PUT /um/users/{id}` — update a user.
    pub async fn update_user(
        &self,
        user_id: &str,
        body: &UserUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<User> {
        self.put_json(&user_path(self, Some(user_id)), body, query, etag)
            .await
    }

    /// `DELETE /um/users/{id}` — delete a user.
    pub async fn delete_user(
        &self,
        user_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(&user_path(self, Some(user_id)), query, etag)
            .await
    }

    /// `GET /um/users/{id}/groups` — list groups the user belongs to.
    pub async fn user_groups(&self, user_id: &str, query: &ListQuery) -> Result<Collection<Group>> {
        self.get(&format!("{}/groups", user_path(self, Some(user_id))), query)
            .await
    }

    /// `GET /um/users/{id}/owns` — list resources owned by the user.
    pub async fn user_owns(
        &self,
        user_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<UmResource>> {
        self.get(&format!("{}/owns", user_path(self, Some(user_id))), query)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_user() {
        let json = r#"{
            "id":"u-1",
            "type":"user",
            "properties":{"email":"a@b.c","administrator":true,"active":true},
            "metadata":{"lastLogin":"2015-12-04T14:34:09.809Z"}
        }"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.id.as_deref(), Some("u-1"));
        assert_eq!(
            user.properties.as_ref().unwrap().email.as_deref(),
            Some("a@b.c")
        );
    }
}
