//! Small Authentik API v3 client for users, groups, applications, and OAuth2 providers.
//! The caller supplies an API token. List methods follow Authentik pagination.

use reqwest::{Method, StatusCode, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, AuthentikError>;

#[derive(Debug, thiserror::Error)]
pub enum AuthentikError {
    #[error("invalid Authentik base URL: {0}")]
    InvalidBaseUrl(String),
    #[error("invalid Authentik URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("Authentik HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Authentik JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Authentik API error {status}: {message}")]
    Api { status: u16, message: String },
    #[error("invalid Authentik pagination: next page {next} after page {current}")]
    Pagination { current: u64, next: u64 },
}

#[derive(Clone)]
pub struct AuthentikClient {
    http: reqwest::Client,
    base: Url,
    token: String,
}

impl AuthentikClient {
    /// `base_url` is the instance root, for example `https://auth.example`.
    pub fn new(base_url: impl AsRef<str>, token: impl Into<String>) -> Result<Self> {
        let mut base = Url::parse(base_url.as_ref())?;
        if !matches!(base.scheme(), "http" | "https") || base.host().is_none() {
            return Err(AuthentikError::InvalidBaseUrl(base_url.as_ref().to_owned()));
        }
        // Accept both an instance root and a URL ending in /api/v3/.
        let path = base.path().trim_end_matches('/').to_string();
        let path = path.strip_suffix("/api/v3").unwrap_or(&path);
        base.set_path(&format!("{}/api/v3/", path.trim_end_matches('/')));
        base.set_query(None);
        base.set_fragment(None);
        Ok(Self {
            http: reqwest::Client::new(),
            base,
            token: token.into(),
        })
    }

    fn url(&self, path: &[&str]) -> Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("HTTP URL has path segments")
            .pop_if_empty()
            .extend(path);
        url
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &[&str],
        query: &[(&str, &str)],
        body: Option<&impl Serialize>,
    ) -> Result<T> {
        let mut req = self
            .http
            .request(method, self.url(path))
            .bearer_auth(&self.token)
            .header("Accept", "application/json")
            .query(query);
        if let Some(body) = body {
            req = req.json(body);
        }
        let res = req.send().await?;
        let status = res.status();
        let bytes = res.bytes().await?;
        if !status.is_success() {
            return Err(api_error(status, &bytes));
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    async fn request_optional<T: DeserializeOwned>(&self, path: &[&str]) -> Result<Option<T>> {
        let res = self
            .http
            .get(self.url(path))
            .bearer_auth(&self.token)
            .header("Accept", "application/json")
            .send()
            .await?;
        let status = res.status();
        let bytes = res.bytes().await?;
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(api_error(status, &bytes));
        }
        Ok(Some(serde_json::from_slice(&bytes)?))
    }

    async fn list<T: DeserializeOwned>(
        &self,
        path: &[&str],
        filter: &[(&str, &str)],
    ) -> Result<Vec<T>> {
        let mut out = Vec::new();
        let mut page = 1_u64;
        loop {
            let page_str = page.to_string();
            let mut query = filter.to_vec();
            query.push(("page", &page_str));
            let response: Page<T> = self.request(Method::GET, path, &query, None::<&()>).await?;
            out.extend(response.results);
            let next = response.pagination.next;
            if next == 0 {
                break;
            }
            if next <= page {
                return Err(AuthentikError::Pagination {
                    current: page,
                    next,
                });
            }
            page = next;
        }
        Ok(out)
    }

    pub async fn users(&self, username: &str) -> Result<Vec<User>> {
        self.list(&["core", "users", ""], &[("username", username)])
            .await
    }
    pub async fn user(&self, id: i64) -> Result<Option<User>> {
        self.request_optional(&["core", "users", &id.to_string(), ""])
            .await
    }
    pub async fn create_user(&self, body: &UserWrite) -> Result<User> {
        self.request(Method::POST, &["core", "users", ""], &[], Some(body))
            .await
    }
    pub async fn patch_user(&self, id: i64, body: &serde_json::Value) -> Result<User> {
        self.request(
            Method::PATCH,
            &["core", "users", &id.to_string(), ""],
            &[],
            Some(body),
        )
        .await
    }
    pub async fn groups(&self, name: &str) -> Result<Vec<Group>> {
        self.list(&["core", "groups", ""], &[("name", name)]).await
    }
    pub async fn group(&self, id: &str) -> Result<Option<Group>> {
        self.request_optional(&["core", "groups", id, ""]).await
    }
    pub async fn create_group(&self, body: &GroupWrite) -> Result<Group> {
        self.request(Method::POST, &["core", "groups", ""], &[], Some(body))
            .await
    }
    pub async fn patch_group(&self, id: &str, body: &serde_json::Value) -> Result<Group> {
        self.request(Method::PATCH, &["core", "groups", id, ""], &[], Some(body))
            .await
    }
    pub async fn application(&self, slug: &str) -> Result<Option<Application>> {
        self.request_optional(&["core", "applications", slug, ""])
            .await
    }
    pub async fn create_application(&self, body: &ApplicationWrite) -> Result<Application> {
        self.request(Method::POST, &["core", "applications", ""], &[], Some(body))
            .await
    }
    pub async fn patch_application(
        &self,
        slug: &str,
        body: &serde_json::Value,
    ) -> Result<Application> {
        self.request(
            Method::PATCH,
            &["core", "applications", slug, ""],
            &[],
            Some(body),
        )
        .await
    }
    pub async fn oauth2_providers(&self, name: &str) -> Result<Vec<OAuth2Provider>> {
        self.list(&["providers", "oauth2", ""], &[("name", name)])
            .await
    }
    pub async fn oauth2_provider(&self, id: i64) -> Result<Option<OAuth2Provider>> {
        self.request_optional(&["providers", "oauth2", &id.to_string(), ""])
            .await
    }
    pub async fn create_oauth2_provider(
        &self,
        body: &OAuth2ProviderWrite,
    ) -> Result<OAuth2Provider> {
        self.request(Method::POST, &["providers", "oauth2", ""], &[], Some(body))
            .await
    }
    pub async fn patch_oauth2_provider(
        &self,
        id: i64,
        body: &serde_json::Value,
    ) -> Result<OAuth2Provider> {
        self.request(
            Method::PATCH,
            &["providers", "oauth2", &id.to_string(), ""],
            &[],
            Some(body),
        )
        .await
    }
}

fn api_error(status: StatusCode, bytes: &[u8]) -> AuthentikError {
    let value: Option<serde_json::Value> = serde_json::from_slice(bytes).ok();
    let message = value
        .and_then(|v| v.get("detail").and_then(|x| x.as_str()).map(str::to_owned))
        .unwrap_or_else(|| String::from_utf8_lossy(bytes).trim().to_owned());
    AuthentikError::Api {
        status: status.as_u16(),
        message,
    }
}

#[derive(Deserialize)]
struct Page<T> {
    pagination: Pagination,
    results: Vec<T>,
}
#[derive(Deserialize)]
struct Pagination {
    next: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct User {
    pub pk: i64,
    pub username: String,
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub groups: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct UserWrite {
    pub username: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<String>>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Group {
    pub pk: String,
    pub name: String,
    #[serde(default)]
    pub is_superuser: bool,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub users: Vec<i64>,
}
#[derive(Clone, Debug, Serialize)]
pub struct GroupWrite {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_superuser: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<i64>>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Application {
    pub pk: String,
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub provider: Option<i64>,
    #[serde(default)]
    pub meta_description: String,
    #[serde(default)]
    pub meta_publisher: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct ApplicationWrite {
    pub slug: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_publisher: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct RedirectUri {
    pub matching_mode: String,
    pub url: String,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct OAuth2Provider {
    pub pk: i64,
    pub name: String,
    pub authorization_flow: String,
    pub invalidation_flow: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default)]
    pub client_type: String,
    #[serde(default)]
    pub redirect_uris: Vec<RedirectUri>,
}
impl std::fmt::Debug for OAuth2Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuth2Provider")
            .field("pk", &self.pk)
            .field("name", &self.name)
            .field("authorization_flow", &self.authorization_flow)
            .field("invalidation_flow", &self.invalidation_flow)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[redacted]")
            .field("client_type", &self.client_type)
            .field("redirect_uris", &self.redirect_uris)
            .finish()
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct OAuth2ProviderWrite {
    pub name: String,
    pub authorization_flow: String,
    pub invalidation_flow: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uris: Option<Vec<RedirectUri>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn mock(responses: Vec<&'static str>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = Vec::new();
                loop {
                    let mut chunk = [0; 2048];
                    let n = socket.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&buf[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|v| v.parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if buf.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8_lossy(&buf).to_string());
                let body = response.as_bytes();
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await.unwrap();
                socket.write_all(body).await.unwrap();
            }
            requests
        });
        (url, task)
    }

    #[tokio::test]
    async fn list_follows_pages_and_uses_bearer_auth() {
        let (url, task) = mock(vec![
            r#"{"pagination":{"next":2},"results":[{"pk":1,"username":"alice","name":"Alice"}]}"#,
            r#"{"pagination":{"next":0},"results":[{"pk":2,"username":"alice","name":"Other Alice"}]}"#,
        ]).await;
        let client = AuthentikClient::new(&url, "secret").unwrap();
        assert_eq!(client.users("alice").await.unwrap().len(), 2);
        let req = task.await.unwrap();
        assert!(
            req[0].starts_with("GET /api/v3/core/users/?username=alice&page=1 HTTP/1.1"),
            "{}",
            req[0]
        );
        assert!(
            req[1].starts_with("GET /api/v3/core/users/?username=alice&page=2 HTTP/1.1"),
            "{}",
            req[1]
        );
        assert!(req[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer secret"));
    }

    #[tokio::test]
    async fn create_and_patch_application_use_api_v3_paths() {
        let body = r#"{"pk":"app-uuid","slug":"my-app","name":"My App","provider":3}"#;
        let (url, task) = mock(vec![body, body]).await;
        let client = AuthentikClient::new(format!("{url}/api/v3/"), "token").unwrap();
        let app = client
            .create_application(&ApplicationWrite {
                slug: "my-app".into(),
                name: "My App".into(),
                provider: Some(3),
                meta_description: None,
                meta_publisher: None,
            })
            .await
            .unwrap();
        assert_eq!(app.slug, "my-app");
        client
            .patch_application("my-app", &serde_json::json!({"name":"My App"}))
            .await
            .unwrap();
        let req = task.await.unwrap();
        assert!(
            req[0].starts_with("POST /api/v3/core/applications/ HTTP/1.1"),
            "{}",
            req[0]
        );
        assert!(req[0].contains("\"provider\":3"));
        assert!(
            req[1].starts_with("PATCH /api/v3/core/applications/my-app/ HTTP/1.1"),
            "{}",
            req[1]
        );
        assert!(!req[1].contains("provider"));
    }

    #[test]
    fn rejects_non_http_base_url() {
        assert!(AuthentikClient::new("file:///tmp/no", "token").is_err());
    }
}
