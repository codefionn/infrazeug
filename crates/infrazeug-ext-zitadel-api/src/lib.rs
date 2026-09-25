//! Small typed client for ZITADEL's Management v1 REST API.
//!
//! The API is deprecated upstream in favor of resource v2 services, but its REST
//! routes cover projects, OIDC apps, roles, and human users consistently.

use reqwest::Method;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, ZitadelError>;

#[derive(Debug, thiserror::Error)]
pub enum ZitadelError {
    #[error("ZITADEL HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("ZITADEL JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ZITADEL API error {status}: {message}")]
    Api { status: u16, message: String },
    #[error("invalid ZITADEL base URL: {0}")]
    Url(String),
}

#[derive(Clone)]
pub struct ZitadelClient {
    base_url: String,
    token: String,
    http: reqwest::Client,
}

impl ZitadelClient {
    /// `token` is a personal access token or another bearer access token with
    /// management permissions. No token is refreshed by this client.
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Result<Self> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let url = reqwest::Url::parse(&base_url).map_err(|e| ZitadelError::Url(e.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(ZitadelError::Url(base_url));
        }
        Ok(Self {
            base_url,
            token: token.into(),
            http: reqwest::Client::new(),
        })
    }

    async fn request<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T> {
        let url = format!("{}/management/v1{}", self.base_url, path);
        let mut req = self.http.request(method, url).bearer_auth(&self.token);
        if let Some(body) = body {
            req = req.json(body);
        }
        let response = req.send().await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if !status.is_success() {
            let message = serde_json::from_slice::<ApiError>(&bytes)
                .ok()
                .and_then(|e| e.message)
                .unwrap_or_else(|| String::from_utf8_lossy(&bytes).into_owned());
            return Err(ZitadelError::Api {
                status: status.as_u16(),
                message,
            });
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    async fn search<T: DeserializeOwned>(&self, path: &str) -> Result<Vec<T>> {
        let mut all = Vec::new();
        loop {
            let query =
                serde_json::json!({"query": {"offset": all.len().to_string(), "limit": 100}});
            let page: SearchPage<T> = self.request(Method::POST, path, Some(&query)).await?;
            let size = page.result.len();
            let total = page
                .details
                .as_ref()
                .and_then(|d| d.total_result.as_ref())
                .and_then(|v| {
                    v.as_u64()
                        .map(|n| n as usize)
                        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                });
            all.extend(page.result);
            if size == 0 || total.is_some_and(|n| all.len() >= n) {
                break;
            }
        }
        Ok(all)
    }

    pub async fn projects(&self) -> Result<Vec<Project>> {
        self.search("/projects/_search").await
    }
    pub async fn project(&self, id: &str) -> Result<Project> {
        let r: ProjectResponse = self
            .request(
                Method::GET,
                &format!("/projects/{}", segment(id)),
                None::<&()>,
            )
            .await?;
        Ok(r.project)
    }
    pub async fn create_project(&self, input: &ProjectInput) -> Result<String> {
        let r: ProjectCreated = self.request(Method::POST, "/projects", Some(input)).await?;
        Ok(r.id)
    }
    pub async fn update_project(&self, id: &str, input: &ProjectInput) -> Result<()> {
        let _: serde_json::Value = self
            .request(
                Method::PUT,
                &format!("/projects/{}", segment(id)),
                Some(input),
            )
            .await?;
        Ok(())
    }

    pub async fn apps(&self, project_id: &str) -> Result<Vec<App>> {
        self.search(&format!("/projects/{}/apps/_search", segment(project_id)))
            .await
    }
    pub async fn app(&self, project_id: &str, app_id: &str) -> Result<App> {
        let r: AppResponse = self
            .request(
                Method::GET,
                &format!("/projects/{}/apps/{}", segment(project_id), segment(app_id)),
                None::<&()>,
            )
            .await?;
        Ok(r.app)
    }
    pub async fn create_oidc_app(
        &self,
        project_id: &str,
        input: &OidcAppInput,
    ) -> Result<OidcAppCreated> {
        self.request(
            Method::POST,
            &format!("/projects/{}/apps/oidc", segment(project_id)),
            Some(input),
        )
        .await
    }
    pub async fn update_app_name(&self, project_id: &str, app_id: &str, name: &str) -> Result<()> {
        let _: serde_json::Value = self
            .request(
                Method::PUT,
                &format!("/projects/{}/apps/{}", segment(project_id), segment(app_id)),
                Some(&serde_json::json!({"name": name})),
            )
            .await?;
        Ok(())
    }
    pub async fn update_oidc_config(
        &self,
        project_id: &str,
        app_id: &str,
        input: &OidcConfigInput,
    ) -> Result<()> {
        let path = format!("/projects/{}/apps/{}", segment(project_id), segment(app_id));
        let existing: serde_json::Value = self.request(Method::GET, &path, None::<&()>).await?;
        let mut config = existing
            .get("app")
            .and_then(|v| v.get("oidcConfig"))
            .cloned()
            .ok_or_else(|| ZitadelError::Api {
                status: 400,
                message: "application has no OIDC config".into(),
            })?;
        let managed = serde_json::to_value(input)?;
        if let (Some(target), Some(changes)) = (config.as_object_mut(), managed.as_object()) {
            const WRITABLE: &[&str] = &[
                "redirectUris",
                "responseTypes",
                "grantTypes",
                "appType",
                "authMethodType",
                "postLogoutRedirectUris",
                "devMode",
                "accessTokenType",
                "accessTokenRoleAssertion",
                "idTokenRoleAssertion",
                "idTokenUserinfoAssertion",
                "clockSkew",
                "additionalOrigins",
                "skipNativeAppSuccessPage",
                "backChannelLogoutUri",
                "loginVersion",
                "ios",
                "android",
            ];
            target.retain(|key, _| WRITABLE.contains(&key.as_str()));
            for (key, value) in changes {
                target.insert(key.clone(), value.clone());
            }
        }
        let _: serde_json::Value = self
            .request(Method::PUT, &format!("{path}/oidc_config"), Some(&config))
            .await?;
        Ok(())
    }

    pub async fn roles(&self, project_id: &str) -> Result<Vec<ProjectRole>> {
        self.search(&format!("/projects/{}/roles/_search", segment(project_id)))
            .await
    }
    pub async fn create_role(&self, project_id: &str, input: &ProjectRoleInput) -> Result<()> {
        let _: serde_json::Value = self
            .request(
                Method::POST,
                &format!("/projects/{}/roles", segment(project_id)),
                Some(input),
            )
            .await?;
        Ok(())
    }
    pub async fn update_role(
        &self,
        project_id: &str,
        key: &str,
        input: &ProjectRoleUpdate,
    ) -> Result<()> {
        let _: serde_json::Value = self
            .request(
                Method::PUT,
                &format!("/projects/{}/roles/{}", segment(project_id), segment(key)),
                Some(input),
            )
            .await?;
        Ok(())
    }

    pub async fn users(&self) -> Result<Vec<User>> {
        self.search("/users/_search").await
    }
    pub async fn user(&self, id: &str) -> Result<User> {
        let r: UserResponse = self
            .request(Method::GET, &format!("/users/{}", segment(id)), None::<&()>)
            .await?;
        Ok(r.user)
    }
    pub async fn create_human_user(&self, input: &HumanUserInput) -> Result<String> {
        let r: UserCreated = self
            .request(Method::POST, "/users/human", Some(input))
            .await?;
        Ok(r.user_id)
    }
    pub async fn update_human_profile(&self, id: &str, profile: &HumanProfile) -> Result<()> {
        let path = format!("/users/{}", segment(id));
        let existing: serde_json::Value = self.request(Method::GET, &path, None::<&()>).await?;
        let mut body = existing
            .get("user")
            .and_then(|v| v.get("human"))
            .and_then(|v| v.get("profile"))
            .cloned()
            .ok_or_else(|| ZitadelError::Api {
                status: 400,
                message: "user has no human profile".into(),
            })?;
        if let Some(object) = body.as_object_mut() {
            object.remove("avatarUrl");
            object.insert("firstName".into(), profile.first_name.clone().into());
            object.insert("lastName".into(), profile.last_name.clone().into());
            object.insert("displayName".into(), profile.display_name.clone().into());
        }
        let _: serde_json::Value = self
            .request(Method::PUT, &format!("{path}/profile"), Some(&body))
            .await?;
        Ok(())
    }
    pub async fn update_human_email(&self, id: &str, email: &str) -> Result<()> {
        let _: serde_json::Value = self
            .request(
                Method::PUT,
                &format!("/users/{}/email", segment(id)),
                Some(&serde_json::json!({"email": email})),
            )
            .await?;
        Ok(())
    }
}

fn segment(value: &str) -> String {
    urlencoding::encode(value).into_owned()
}

#[derive(Deserialize)]
struct ApiError {
    message: Option<String>,
}
#[derive(Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct SearchPage<T> {
    #[serde(default)]
    result: Vec<T>,
    details: Option<SearchDetails>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchDetails {
    total_result: Option<serde_json::Value>,
}
#[derive(Deserialize)]
struct ProjectResponse {
    project: Project,
}
#[derive(Deserialize)]
struct ProjectCreated {
    id: String,
}
#[derive(Deserialize)]
struct AppResponse {
    app: App,
}
#[derive(Deserialize)]
struct UserResponse {
    user: User,
}
#[derive(Deserialize)]
struct UserCreated {
    #[serde(rename = "userId")]
    user_id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_role_assertion: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_role_check: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_project_check: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_labeling_setting: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub project_role_assertion: bool,
    #[serde(default)]
    pub project_role_check: bool,
    #[serde(default)]
    pub has_project_check: bool,
    pub private_labeling_setting: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OidcConfigInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uris: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_logout_redirect_uris: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_method_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_types: Option<Vec<String>>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OidcAppInput {
    pub name: String,
    #[serde(flatten)]
    pub config: OidcConfigInput,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OidcAppCreated {
    pub app_id: String,
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub oidc_config: Option<OidcConfig>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OidcConfig {
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
    #[serde(default)]
    pub post_logout_redirect_uris: Vec<String>,
    pub app_type: Option<String>,
    pub auth_method_type: Option<String>,
    #[serde(default)]
    pub grant_types: Vec<String>,
    #[serde(default)]
    pub response_types: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRoleInput {
    pub role_key: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRoleUpdate {
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRole {
    pub key: String,
    pub display_name: String,
    pub group: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HumanProfile {
    pub first_name: String,
    pub last_name: String,
    pub display_name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HumanEmail {
    pub email: String,
    #[serde(default)]
    pub is_email_verified: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HumanUserInput {
    pub user_name: String,
    pub profile: HumanProfile,
    pub email: HumanEmail,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Human {
    pub profile: HumanProfile,
    pub email: HumanEmail,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub user_name: String,
    pub human: Option<Human>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    #[test]
    fn rejects_bad_url() {
        assert!(ZitadelClient::new("zitadel", "token").is_err());
    }
    #[test]
    fn encodes_path_segment() {
        assert_eq!(segment("a/b"), "a%2Fb");
    }

    #[tokio::test]
    async fn project_search_uses_total_result_for_pagination() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (id, name) in [("one", "alpha"), ("two", "beta")] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0u8; 1024];
                    let n = socket.read(&mut chunk).await.unwrap();
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let end = end + 4;
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|v| v.parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8_lossy(&bytes).into_owned());
                let body = format!(
                    r#"{{"details":{{"totalResult":2}},"result":[{{"id":"{id}","name":"{name}"}}]}}"#
                );
                let reply = format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", body.len(), body);
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
            requests
        });
        let client = ZitadelClient::new(format!("http://{address}"), "pat").unwrap();
        let projects = client.projects().await.unwrap();
        assert_eq!(
            projects.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["alpha", "beta"]
        );
        let requests = server.await.unwrap();
        assert!(requests[0].contains("\"offset\":\"0\""));
        assert!(requests[1].contains("\"offset\":\"1\""));
        assert!(requests
            .iter()
            .all(|r| r.starts_with("POST /management/v1/projects/_search")));
    }

    #[tokio::test]
    async fn oidc_update_preserves_unmanaged_config() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for body in [
                r#"{"app":{"id":"app","name":"web","oidcConfig":{"clientId":"generated","redirectUris":["https://old.example/cb"],"devMode":true,"accessTokenType":"OIDC_TOKEN_TYPE_JWT"}}}"#,
                r#"{"details":{}}"#,
            ] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0u8; 1024];
                    let n = socket.read(&mut chunk).await.unwrap();
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let end = end + 4;
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|v| v.parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8_lossy(&bytes).into_owned());
                let reply = format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", body.len(), body);
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
            requests
        });
        let client = ZitadelClient::new(format!("http://{address}"), "pat").unwrap();
        client
            .update_oidc_config(
                "project",
                "app",
                &OidcConfigInput {
                    redirect_uris: Some(vec!["https://new.example/cb".into()]),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let requests = server.await.unwrap();
        assert!(requests[0].starts_with("GET /management/v1/projects/project/apps/app"));
        assert!(requests[1].starts_with("PUT /management/v1/projects/project/apps/app/oidc_config"));
        let body: serde_json::Value =
            serde_json::from_str(requests[1].split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(
            body["redirectUris"],
            serde_json::json!(["https://new.example/cb"])
        );
        assert_eq!(body["devMode"], true);
        assert_eq!(body["accessTokenType"], "OIDC_TOKEN_TYPE_JWT");
        assert!(body.get("clientId").is_none());
    }
}
