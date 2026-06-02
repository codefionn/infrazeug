//! Public Cloud **users** and S3 credentials (`/cloud/project/…/user`).
//!
//! Field/route shapes follow the live v1 schema (`eu.api.ovh.com/1.0/cloud.json`):
//! the user `id` is a number, the list routes return objects (not bare ids), and
//! S3 credentials live under `…/s3Credentials` (plural) as
//! `cloud.user.S3Credentials` (`access` = access-key id, `secret` = the secret,
//! returned only at creation).

use super::project_path;
use crate::client::OvhClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Body for `POST /cloud/project/{serviceName}/user` (`cloud.ProjectUserCreation`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudProjectUserCreate {
    pub description: String,
    /// IAM role names (`cloud.user.RoleEnum`), e.g. `objectstore_operator`.
    pub roles: Vec<String>,
}

/// Public Cloud project user (`cloud.user.User` / `cloud.user.UserDetail`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudProjectUser {
    /// Numeric user id (used as `{userId}` in paths).
    pub id: i64,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub creation_date: Option<String>,
    #[serde(default)]
    pub openstack_id: Option<String>,
    #[serde(default)]
    pub roles: Vec<CloudProjectUserRole>,
    #[serde(default)]
    pub status: Option<String>,
}

/// Role attached to a project user (`cloud.role.Role`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudProjectUserRole {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// S3 credential summary (`cloud.user.S3Credentials`). `access` is the access-key id.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct S3Credential {
    /// Access-key id.
    pub access: String,
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

/// Newly issued S3 credential (`cloud.user.S3CredentialsWithSecret`). The `secret`
/// is returned only on creation, never on later reads.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct S3CredentialSecret {
    /// Access-key id.
    pub access: String,
    /// Secret access key (present only in the creation response).
    pub secret: String,
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

/// S3 IAM policy attached to a Public Cloud project user.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudProjectUserS3Policy {
    /// JSON policy document, serialized as a string by the OVH v1 API.
    pub policy: String,
}

impl OvhClient {
    /// `GET /cloud/project/{serviceName}/user` — list users (`cloud.user.User[]`).
    pub async fn cloud_project_users(&self, service_name: &str) -> Result<Vec<CloudProjectUser>> {
        let path = project_path(service_name, self, "/user");
        self.get_v1(&path).await
    }

    /// `POST /cloud/project/{serviceName}/user` — create a user.
    pub async fn cloud_project_user_create(
        &self,
        service_name: &str,
        create: &CloudProjectUserCreate,
    ) -> Result<CloudProjectUser> {
        let path = project_path(service_name, self, "/user");
        self.post_v1(&path, create).await
    }

    /// `GET /cloud/project/{serviceName}/user/{userId}` — user detail.
    pub async fn cloud_project_user(
        &self,
        service_name: &str,
        user_id: &str,
    ) -> Result<CloudProjectUser> {
        let path = format!(
            "{}/{}",
            project_path(service_name, self, "/user"),
            self.encode_segment(user_id)
        );
        self.get_v1(&path).await
    }

    /// `DELETE /cloud/project/{serviceName}/user/{userId}`.
    pub async fn cloud_project_user_delete(&self, service_name: &str, user_id: &str) -> Result<()> {
        let path = format!(
            "{}/{}",
            project_path(service_name, self, "/user"),
            self.encode_segment(user_id)
        );
        self.delete_v1(&path).await
    }

    /// `GET /cloud/project/{serviceName}/user/{userId}/s3Credentials` — list S3
    /// credentials (`cloud.user.S3Credentials[]`).
    pub async fn cloud_project_user_s3_credentials(
        &self,
        service_name: &str,
        user_id: &str,
    ) -> Result<Vec<S3Credential>> {
        let path = format!(
            "{}/{}/s3Credentials",
            project_path(service_name, self, "/user"),
            self.encode_segment(user_id)
        );
        self.get_v1(&path).await
    }

    /// `POST /cloud/project/{serviceName}/user/{userId}/s3Credentials` — issue S3
    /// keys (`cloud.user.S3CredentialsWithSecret`, the only time `secret` is shown).
    pub async fn cloud_project_user_s3_credential_create(
        &self,
        service_name: &str,
        user_id: &str,
    ) -> Result<S3CredentialSecret> {
        let path = format!(
            "{}/{}/s3Credentials",
            project_path(service_name, self, "/user"),
            self.encode_segment(user_id)
        );
        // This route rejects any request body (even `{}`), so POST bodyless.
        self.post_v1_no_body(&path).await
    }

    /// `GET /cloud/project/{serviceName}/user/{userId}/policy` — read the user's
    /// Object Storage S3 IAM policy.
    pub async fn cloud_project_user_s3_policy(
        &self,
        service_name: &str,
        user_id: &str,
    ) -> Result<CloudProjectUserS3Policy> {
        let path = format!(
            "{}/{}/policy",
            project_path(service_name, self, "/user"),
            self.encode_segment(user_id)
        );
        self.get_v1(&path).await
    }

    /// `POST /cloud/project/{serviceName}/user/{userId}/policy` — set the user's
    /// Object Storage S3 IAM policy.
    pub async fn cloud_project_user_s3_policy_set(
        &self,
        service_name: &str,
        user_id: &str,
        policy: &CloudProjectUserS3Policy,
    ) -> Result<()> {
        let path = format!(
            "{}/{}/policy",
            project_path(service_name, self, "/user"),
            self.encode_segment(user_id)
        );
        self.post_v1(&path, policy).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_user_with_numeric_id() {
        let u: CloudProjectUser = serde_json::from_str(
            r#"{
                "id": 4242,
                "username": "user-abc",
                "description": "infrazeug-cnpg-backup",
                "creationDate": "2024-01-01T00:00:00+00:00",
                "status": "ok",
                "roles": [{"id": "r1", "name": "objectstore_operator"}]
            }"#,
        )
        .unwrap();
        assert_eq!(u.id, 4242);
        assert_eq!(u.description.as_deref(), Some("infrazeug-cnpg-backup"));
        assert_eq!(u.roles[0].name, "objectstore_operator");
    }

    #[test]
    fn deserializes_s3_credential_list_entry() {
        let c: S3Credential =
            serde_json::from_str(r#"{"access": "ak", "tenantId": "t", "userId": "4242"}"#).unwrap();
        assert_eq!(c.access, "ak");
    }

    #[test]
    fn deserializes_s3_credential_with_secret() {
        let c: S3CredentialSecret =
            serde_json::from_str(r#"{"access": "ak", "secret": "sk", "userId": "4242"}"#).unwrap();
        assert_eq!(c.access, "ak");
        assert_eq!(c.secret, "sk");
    }

    #[test]
    fn serializes_user_create_with_roles() {
        let body = CloudProjectUserCreate {
            description: "backup".into(),
            roles: vec!["objectstore_operator".into()],
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains(r#""roles":["objectstore_operator"]"#));
    }

    #[test]
    fn serializes_s3_policy_as_string() {
        let body = CloudProjectUserS3Policy {
            policy: r#"{"Statement":[]}"#.into(),
        };
        let json = serde_json::to_string(&body).unwrap();
        assert_eq!(json, r#"{"policy":"{\"Statement\":[]}"}"#);
    }
}
