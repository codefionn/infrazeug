//! Group management (`/um/groups`).

use crate::client::IonosClient;
use crate::error::Result;
use crate::types::{Collection, ListQuery};

pub use crate::um_types::{
    Group, GroupCreate, GroupMemberRef, GroupProperties, GroupShare, GroupShareProperties,
    GroupShareUpdate, GroupUpdate,
};
use crate::um_types::{UmResource, User};

fn group_path(client: &IonosClient, group_id: Option<&str>) -> String {
    let mut path = "/um/groups".to_string();
    if let Some(group_id) = group_id {
        path.push('/');
        path.push_str(&client.encode_path(group_id));
    }
    path
}

fn group_share_path(client: &IonosClient, group_id: &str, resource_id: Option<&str>) -> String {
    let mut path = format!("{}/shares", group_path(client, Some(group_id)));
    if let Some(resource_id) = resource_id {
        path.push('/');
        path.push_str(&client.encode_path(resource_id));
    }
    path
}

impl IonosClient {
    /// `GET /um/groups` — list all groups.
    pub async fn groups(&self, query: &ListQuery) -> Result<Collection<Group>> {
        self.get(&group_path(self, None), query).await
    }

    /// `GET /um/groups/{id}` — retrieve one group.
    pub async fn group(&self, group_id: &str, query: &ListQuery) -> Result<Group> {
        self.get(&group_path(self, Some(group_id)), query).await
    }

    /// `POST /um/groups` — create a group.
    pub async fn create_group(&self, body: &GroupCreate, query: &ListQuery) -> Result<Group> {
        self.post_json(&group_path(self, None), body, query).await
    }

    /// `PUT /um/groups/{id}` — update a group.
    pub async fn update_group(
        &self,
        group_id: &str,
        body: &GroupUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<Group> {
        self.put_json(&group_path(self, Some(group_id)), body, query, etag)
            .await
    }

    /// `DELETE /um/groups/{id}` — delete a group.
    pub async fn delete_group(
        &self,
        group_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(&group_path(self, Some(group_id)), query, etag)
            .await
    }

    /// `GET /um/groups/{id}/resources` — list resources assigned to a group.
    pub async fn group_resources(
        &self,
        group_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<UmResource>> {
        self.get(
            &format!("{}/resources", group_path(self, Some(group_id))),
            query,
        )
        .await
    }

    /// `GET /um/groups/{id}/users` — list group members.
    pub async fn group_members(
        &self,
        group_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<User>> {
        self.get(
            &format!("{}/users", group_path(self, Some(group_id))),
            query,
        )
        .await
    }

    /// `POST /um/groups/{id}/users` — add an existing user to a group.
    pub async fn add_group_member(
        &self,
        group_id: &str,
        user_id: &str,
        query: &ListQuery,
    ) -> Result<User> {
        let body = GroupMemberRef {
            id: user_id.to_string(),
        };
        self.post_json(
            &format!("{}/users", group_path(self, Some(group_id))),
            &body,
            query,
        )
        .await
    }

    /// `DELETE /um/groups/{id}/users/{userId}` — remove a user from a group.
    pub async fn remove_group_member(
        &self,
        group_id: &str,
        user_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &format!(
                "{}/users/{}",
                group_path(self, Some(group_id)),
                self.encode_path(user_id)
            ),
            query,
            etag,
        )
        .await
    }

    /// `GET /um/groups/{id}/shares` — list resource shares for a group.
    pub async fn group_shares(
        &self,
        group_id: &str,
        query: &ListQuery,
    ) -> Result<Collection<GroupShare>> {
        self.get(&group_share_path(self, group_id, None), query)
            .await
    }

    /// `GET /um/groups/{id}/shares/{resourceId}` — retrieve one share.
    pub async fn group_share(
        &self,
        group_id: &str,
        resource_id: &str,
        query: &ListQuery,
    ) -> Result<GroupShare> {
        self.get(&group_share_path(self, group_id, Some(resource_id)), query)
            .await
    }

    /// `POST /um/groups/{id}/shares/{resourceId}` — share a resource with a group.
    pub async fn add_group_share(
        &self,
        group_id: &str,
        resource_id: &str,
        body: &GroupShareUpdate,
        query: &ListQuery,
    ) -> Result<GroupShare> {
        self.post_json(
            &group_share_path(self, group_id, Some(resource_id)),
            body,
            query,
        )
        .await
    }

    /// `PUT /um/groups/{id}/shares/{resourceId}` — update share privileges.
    pub async fn update_group_share(
        &self,
        group_id: &str,
        resource_id: &str,
        body: &GroupShareUpdate,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<GroupShare> {
        self.put_json(
            &group_share_path(self, group_id, Some(resource_id)),
            body,
            query,
            etag,
        )
        .await
    }

    /// `DELETE /um/groups/{id}/shares/{resourceId}` — remove a resource share.
    pub async fn remove_group_share(
        &self,
        group_id: &str,
        resource_id: &str,
        query: &ListQuery,
        etag: Option<&str>,
    ) -> Result<()> {
        self.delete(
            &group_share_path(self, group_id, Some(resource_id)),
            query,
            etag,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_group_privileges() {
        let json = r#"{
            "id":"g-1",
            "type":"group",
            "properties":{"name":"ops","createDatacenter":true,"manageDBaaS":true}
        }"#;
        let group: Group = serde_json::from_str(json).unwrap();
        assert_eq!(
            group.properties.as_ref().unwrap().name.as_deref(),
            Some("ops")
        );
        assert_eq!(group.properties.as_ref().unwrap().manage_dbaas, Some(true));
    }
}
