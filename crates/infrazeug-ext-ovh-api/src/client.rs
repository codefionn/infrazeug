//! Signed HTTP client for the OVHcloud API (v1 and v2 branches).
//!
//! [`OvhClient`] supports two authentication methods:
//!
//! - **Classic**: synchronises its clock with `/auth/time`, signs every
//!   authenticated request per the OVH scheme (AK/AS/CK + SHA-1 signature).
//! - **OAuth2**: fetches a Bearer token via the OAuth2 client-credentials flow
//!   and sends `Authorization: Bearer <token>` on every request. Tokens are
//!   cached and automatically refreshed before expiry.

use crate::auth::{
    signature, AuthMethod, CachedToken, Credentials, OAuth2Credentials, TokenResponse,
};
use crate::error::{OvhError, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// OVHcloud API region / brand endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OvhEndpoint {
    /// `https://eu.api.ovh.com`
    OvhEu,
    /// `https://api.us.ovhcloud.com`
    OvhUs,
    /// `https://ca.api.ovh.com`
    OvhCa,
    /// `https://eu.api.kimsufi.com`
    KimsufiEu,
    /// `https://ca.api.kimsufi.com`
    KimsufiCa,
    /// `https://eu.api.soyoustart.com`
    SoyoustartEu,
    /// `https://ca.api.soyoustart.com`
    SoyoustartCa,
}

impl OvhEndpoint {
    /// v1 base URL (`…/1.0`).
    pub fn base_url_v1(self) -> &'static str {
        match self {
            Self::OvhEu => "https://eu.api.ovh.com/1.0",
            Self::OvhUs => "https://api.us.ovhcloud.com/1.0",
            Self::OvhCa => "https://ca.api.ovh.com/1.0",
            Self::KimsufiEu => "https://eu.api.kimsufi.com/1.0",
            Self::KimsufiCa => "https://ca.api.kimsufi.com/1.0",
            Self::SoyoustartEu => "https://eu.api.soyoustart.com/1.0",
            Self::SoyoustartCa => "https://ca.api.soyoustart.com/1.0",
        }
    }

    /// v2 base URL (`…/v2`).
    pub fn base_url_v2(self) -> &'static str {
        match self {
            Self::OvhEu => "https://eu.api.ovh.com/v2",
            Self::OvhUs => "https://api.us.ovhcloud.com/v2",
            Self::OvhCa => "https://ca.api.ovh.com/v2",
            Self::KimsufiEu => "https://eu.api.kimsufi.com/v2",
            Self::KimsufiCa => "https://ca.api.kimsufi.com/v2",
            Self::SoyoustartEu => "https://eu.api.soyoustart.com/v2",
            Self::SoyoustartCa => "https://ca.api.soyoustart.com/v2",
        }
    }

    /// OAuth2 token endpoint for this region.
    ///
    /// Only the three primary endpoints (EU, US, CA) support OAuth2 service
    /// accounts. Kimsufi and Soyoustart fall back to their parent region.
    pub fn oauth2_token_url(self) -> &'static str {
        match self {
            Self::OvhEu | Self::KimsufiEu | Self::SoyoustartEu => {
                "https://www.ovh.com/auth/oauth2/token"
            }
            Self::OvhUs => "https://us.ovhcloud.com/auth/oauth2/token",
            Self::OvhCa | Self::KimsufiCa | Self::SoyoustartCa => {
                "https://ca.ovh.com/auth/oauth2/token"
            }
        }
    }
}

/// API branch selector for request routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApiBranch {
    V1,
    V2,
}

/// Optional v2 request headers (schema version, cursor pagination).
#[derive(Debug, Clone, Default)]
pub struct V2RequestOptions<'a> {
    /// `X-Schemas-Version` — pin a major schema revision (e.g. `"1.0"`).
    pub schemas_version: Option<&'a str>,
    /// `X-Pagination-Cursor` — continue a paginated listing.
    pub pagination_cursor: Option<&'a str>,
    /// `X-Pagination-Size` — page size for cursor pagination.
    pub pagination_size: Option<u32>,
}

/// Cursor pagination metadata returned by v2 list endpoints.
#[derive(Debug, Clone, Default)]
pub struct V2PageInfo {
    /// Value for `X-Pagination-Cursor` on the next request, when present.
    pub next_cursor: Option<String>,
}

/// Request parameters for cursor-paginated v2 list endpoints.
#[derive(Debug, Clone, Default)]
pub struct PageParams {
    /// `X-Pagination-Cursor` — continue from a previous response.
    pub cursor: Option<String>,
    /// `X-Pagination-Size` — maximum items per page.
    pub size: Option<u32>,
}

impl PageParams {
    fn to_options(&self) -> V2RequestOptions<'_> {
        V2RequestOptions {
            schemas_version: None,
            pagination_cursor: self.cursor.as_deref(),
            pagination_size: self.size,
        }
    }
}

/// One page of items from a cursor-paginated v2 list endpoint.
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

/// Percent-encode a single path segment (alias for URL encoding).
pub fn percent_encode(segment: &str) -> String {
    urlencoding::encode(segment).into_owned()
}

/// An authenticated OVHcloud API client.
///
/// Construct with [`OvhClient::new`] (classic AK/AS/CK),
/// [`OvhClient::application_only`] (classic, no CK), or
/// [`OvhClient::oauth2`] (OAuth2 service account).
#[derive(Clone)]
pub struct OvhClient {
    pub(crate) http: Client,
    pub(crate) endpoint: OvhEndpoint,
    pub(crate) auth: AuthMethod,
    time_delta: Arc<RwLock<Option<i64>>>,
    oauth2_token: Arc<RwLock<Option<CachedToken>>>,
}

impl OvhClient {
    /// Build a client for the given endpoint and application + consumer credentials.
    pub fn new(
        endpoint: OvhEndpoint,
        application_key: impl Into<String>,
        application_secret: impl Into<String>,
        consumer_key: impl Into<String>,
    ) -> Self {
        Self::from_auth(
            endpoint,
            AuthMethod::Classic(Credentials::new(
                application_key,
                application_secret,
                consumer_key,
            )),
        )
    }

    /// Application-only client (no consumer key) for bootstrapping credentials.
    pub fn application_only(
        endpoint: OvhEndpoint,
        application_key: impl Into<String>,
        application_secret: impl Into<String>,
    ) -> Self {
        Self::from_auth(
            endpoint,
            AuthMethod::Classic(Credentials::application_only(
                application_key,
                application_secret,
            )),
        )
    }

    /// Build a client authenticated via an OAuth2 service account.
    pub fn oauth2(
        endpoint: OvhEndpoint,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        Self::from_auth(
            endpoint,
            AuthMethod::OAuth2(OAuth2Credentials::new(client_id, client_secret)),
        )
    }

    /// Build a client from an existing [`Credentials`] value (classic auth).
    pub fn from_credentials(endpoint: OvhEndpoint, credentials: Credentials) -> Self {
        Self::from_auth(endpoint, AuthMethod::Classic(credentials))
    }

    /// Build a client from an explicit [`AuthMethod`].
    pub fn from_auth(endpoint: OvhEndpoint, auth: AuthMethod) -> Self {
        Self {
            http: Client::new(),
            endpoint,
            auth,
            time_delta: Arc::new(RwLock::new(None)),
            oauth2_token: Arc::new(RwLock::new(None)),
        }
    }

    /// Use a pre-configured [`reqwest::Client`] (custom timeouts, proxy, …).
    pub fn with_http_client(mut self, http: Client) -> Self {
        self.http = http;
        self
    }

    /// Configured API region.
    pub fn endpoint(&self) -> OvhEndpoint {
        self.endpoint
    }

    pub(crate) fn api_url(&self, branch: ApiBranch, path: &str) -> String {
        let base = match branch {
            ApiBranch::V1 => self.endpoint.base_url_v1(),
            ApiBranch::V2 => self.endpoint.base_url_v2(),
        };
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        format!("{base}{path}")
    }

    pub(crate) fn encode_segment(&self, segment: &str) -> String {
        urlencoding::encode(segment).into_owned()
    }

    async fn server_timestamp(&self) -> Result<i64> {
        if let Some(delta) = *self.time_delta.read().await {
            return Ok(unix_now() + delta);
        }

        let (app_key, _) = self.classic_credentials()?;
        let url = self.api_url(ApiBranch::V1, "/auth/time");
        let resp = self
            .http
            .get(&url)
            .header(ACCEPT, "application/json")
            .header("X-Ovh-Application", app_key)
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(api_error(status.as_u16(), &body, None));
        }

        let server_time: i64 = body.trim().parse().map_err(|_| OvhError::Api {
            status: 0,
            code: None,
            message: format!("malformed /auth/time response: {body:?}"),
            query_id: None,
        })?;

        let delta = server_time - unix_now();
        *self.time_delta.write().await = Some(delta);
        Ok(server_time)
    }

    fn classic_credentials(&self) -> Result<(&str, &str)> {
        match &self.auth {
            AuthMethod::Classic(c) => Ok((&c.application_key, &c.application_secret)),
            AuthMethod::OAuth2(_) => Err(OvhError::Api {
                status: 0,
                code: Some("WRONG_AUTH_METHOD".into()),
                message: "this operation requires classic (AK/AS/CK) credentials".into(),
                query_id: None,
            }),
        }
    }

    /// Obtain a valid OAuth2 Bearer token, fetching or refreshing as needed.
    async fn oauth2_bearer(&self) -> Result<String> {
        {
            let guard = self.oauth2_token.read().await;
            if let Some(cached) = guard.as_ref() {
                if cached.is_valid() {
                    return Ok(cached.access_token.clone());
                }
            }
        }

        let oauth2_creds = match &self.auth {
            AuthMethod::OAuth2(c) => c,
            AuthMethod::Classic(_) => unreachable!("oauth2_bearer called with classic auth"),
        };

        let token_url = self.endpoint.oauth2_token_url();
        let resp = self
            .http
            .post(token_url)
            .basic_auth(&oauth2_creds.client_id, Some(&oauth2_creds.client_secret))
            .form(&[("grant_type", "client_credentials"), ("scope", "all")])
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(api_error(status.as_u16(), &body, None));
        }

        let token_resp: TokenResponse = resp.json().await?;
        let cached = CachedToken::from_response(&token_resp);
        let access_token = cached.access_token.clone();
        *self.oauth2_token.write().await = Some(cached);
        Ok(access_token)
    }

    async fn send_request(
        &self,
        branch: ApiBranch,
        method: Method,
        path: &str,
        body: Option<&str>,
        v2: V2RequestOptions<'_>,
        need_auth: bool,
    ) -> Result<reqwest::Response> {
        let url = self.api_url(branch, path);
        let body_str = body.unwrap_or("");

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        if body.is_some() {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }

        if let ApiBranch::V2 = branch {
            if let Some(version) = v2.schemas_version {
                headers.insert(
                    "X-Schemas-Version",
                    HeaderValue::from_str(version).map_err(|e| OvhError::Url(e.to_string()))?,
                );
            }
            if let Some(cursor) = v2.pagination_cursor {
                headers.insert(
                    "X-Pagination-Cursor",
                    HeaderValue::from_str(cursor).map_err(|e| OvhError::Url(e.to_string()))?,
                );
            }
            if let Some(size) = v2.pagination_size {
                headers.insert(
                    "X-Pagination-Size",
                    HeaderValue::from_str(&size.to_string())
                        .map_err(|e| OvhError::Url(e.to_string()))?,
                );
            }
        }

        if need_auth {
            match &self.auth {
                AuthMethod::Classic(creds) => {
                    headers.insert(
                        "X-Ovh-Application",
                        HeaderValue::from_str(&creds.application_key)
                            .map_err(|e| OvhError::Url(e.to_string()))?,
                    );

                    let consumer_key =
                        creds.consumer_key.as_deref().ok_or_else(|| OvhError::Api {
                            status: 0,
                            code: Some("MISSING_CONSUMER_KEY".into()),
                            message: "consumer key required for authenticated OVH API calls".into(),
                            query_id: None,
                        })?;

                    let timestamp = self.server_timestamp().await?;
                    let sig = signature(
                        &creds.application_secret,
                        consumer_key,
                        method.as_str(),
                        &url,
                        body_str,
                        timestamp,
                    );

                    headers.insert(
                        "X-Ovh-Consumer",
                        HeaderValue::from_str(consumer_key)
                            .map_err(|e| OvhError::Url(e.to_string()))?,
                    );
                    headers.insert(
                        "X-Ovh-Timestamp",
                        HeaderValue::from_str(&timestamp.to_string())
                            .map_err(|e| OvhError::Url(e.to_string()))?,
                    );
                    headers.insert(
                        "X-Ovh-Signature",
                        HeaderValue::from_str(&sig).map_err(|e| OvhError::Url(e.to_string()))?,
                    );
                }
                AuthMethod::OAuth2(_) => {
                    let token = self.oauth2_bearer().await?;
                    headers.insert(
                        AUTHORIZATION,
                        HeaderValue::from_str(&format!("Bearer {token}"))
                            .map_err(|e| OvhError::Url(e.to_string()))?,
                    );
                }
            }
        } else if let AuthMethod::Classic(creds) = &self.auth {
            headers.insert(
                "X-Ovh-Application",
                HeaderValue::from_str(&creds.application_key)
                    .map_err(|e| OvhError::Url(e.to_string()))?,
            );
        }

        let mut req = self.http.request(method, &url);
        req = req.headers(headers);
        if let Some(body) = body {
            req = req.body(body.to_string());
        }

        Ok(req.send().await?)
    }

    pub(crate) async fn get_v1<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.get_v1_url(path).await
    }

    pub(crate) async fn get_v1_url<T: DeserializeOwned>(&self, path_and_query: &str) -> Result<T> {
        let resp = self
            .send_request(
                ApiBranch::V1,
                Method::GET,
                path_and_query,
                None,
                V2RequestOptions::default(),
                true,
            )
            .await?;
        decode(resp).await
    }

    pub(crate) async fn post_v1<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let body = serde_json::to_string(body)?;
        let resp = self
            .send_request(
                ApiBranch::V1,
                Method::POST,
                path,
                Some(&body),
                V2RequestOptions::default(),
                true,
            )
            .await?;
        decode(resp).await
    }

    /// POST with no request body, decoding the JSON response into `T`. Some OVH
    /// routes (e.g. issuing S3 credentials) reject any body — sending even `{}`
    /// yields `400 You provided an input body while none was expected`.
    pub(crate) async fn post_v1_no_body<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .send_request(
                ApiBranch::V1,
                Method::POST,
                path,
                None,
                V2RequestOptions::default(),
                true,
            )
            .await?;
        decode(resp).await
    }

    pub(crate) async fn post_v1_void(&self, path: &str) -> Result<()> {
        let resp = self
            .send_request(
                ApiBranch::V1,
                Method::POST,
                path,
                None,
                V2RequestOptions::default(),
                true,
            )
            .await?;
        let (status, text) = consume(resp).await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(api_error(status.as_u16(), &text, None))
        }
    }

    pub(crate) async fn post_v1_void_body<B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<()> {
        let body = serde_json::to_string(body)?;
        let resp = self
            .send_request(
                ApiBranch::V1,
                Method::POST,
                path,
                Some(&body),
                V2RequestOptions::default(),
                true,
            )
            .await?;
        let (status, text) = consume(resp).await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(api_error(status.as_u16(), &text, None))
        }
    }

    pub(crate) async fn delete_v1(&self, path: &str) -> Result<()> {
        let resp = self
            .send_request(
                ApiBranch::V1,
                Method::DELETE,
                path,
                None,
                V2RequestOptions::default(),
                true,
            )
            .await?;
        let (status, text) = consume(resp).await?;
        if status.is_success() {
            Ok(())
        } else {
            Err(api_error(status.as_u16(), &text, None))
        }
    }

    pub(crate) async fn put_v1<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<()> {
        let _ = self
            .put_v1_typed::<B, serde_json::Value>(path, body)
            .await?;
        Ok(())
    }

    pub(crate) async fn put_v1_typed<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let body = serde_json::to_string(body)?;
        let resp = self
            .send_request(
                ApiBranch::V1,
                Method::PUT,
                path,
                Some(&body),
                V2RequestOptions::default(),
                true,
            )
            .await?;
        decode(resp).await
    }

    pub(crate) async fn get_v2<T: DeserializeOwned>(
        &self,
        path: &str,
        options: V2RequestOptions<'_>,
    ) -> Result<(T, V2PageInfo)> {
        let resp = self
            .send_request(ApiBranch::V2, Method::GET, path, None, options, true)
            .await?;
        let page = page_info(&resp);
        let value = decode(resp).await?;
        Ok((value, page))
    }

    pub(crate) async fn get_v2_url<T: DeserializeOwned>(
        &self,
        path_and_query: &str,
        options: V2RequestOptions<'_>,
    ) -> Result<(T, V2PageInfo)> {
        let resp = self
            .send_request(
                ApiBranch::V2,
                Method::GET,
                path_and_query,
                None,
                options,
                true,
            )
            .await?;
        let page = page_info(&resp);
        let value = decode(resp).await?;
        Ok((value, page))
    }

    pub(crate) async fn put_v2<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        options: V2RequestOptions<'_>,
    ) -> Result<T> {
        let body = serde_json::to_string(body)?;
        let resp = self
            .send_request(ApiBranch::V2, Method::PUT, path, Some(&body), options, true)
            .await?;
        decode(resp).await
    }

    pub(crate) async fn put_v2_void<B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
        options: V2RequestOptions<'_>,
    ) -> Result<()> {
        let body = serde_json::to_string(body)?;
        let resp = self
            .send_request(ApiBranch::V2, Method::PUT, path, Some(&body), options, true)
            .await?;
        decode_void(resp).await
    }

    pub(crate) async fn post_v2<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        options: V2RequestOptions<'_>,
    ) -> Result<T> {
        let body = serde_json::to_string(body)?;
        let resp = self
            .send_request(
                ApiBranch::V2,
                Method::POST,
                path,
                Some(&body),
                options,
                true,
            )
            .await?;
        decode(resp).await
    }

    pub(crate) async fn post_v2_void<B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
        options: V2RequestOptions<'_>,
    ) -> Result<()> {
        let body = serde_json::to_string(body)?;
        let resp = self
            .send_request(
                ApiBranch::V2,
                Method::POST,
                path,
                Some(&body),
                options,
                true,
            )
            .await?;
        decode_void(resp).await
    }

    pub(crate) async fn post_v2_no_body<T: DeserializeOwned>(
        &self,
        path: &str,
        options: V2RequestOptions<'_>,
    ) -> Result<T> {
        let resp = self
            .send_request(ApiBranch::V2, Method::POST, path, None, options, true)
            .await?;
        decode(resp).await
    }

    pub(crate) async fn post_v2_no_body_void(
        &self,
        path: &str,
        options: V2RequestOptions<'_>,
    ) -> Result<()> {
        let resp = self
            .send_request(ApiBranch::V2, Method::POST, path, None, options, true)
            .await?;
        decode_void(resp).await
    }

    pub(crate) async fn delete_v2(&self, path: &str, options: V2RequestOptions<'_>) -> Result<()> {
        let resp = self
            .send_request(ApiBranch::V2, Method::DELETE, path, None, options, true)
            .await?;
        decode_void(resp).await
    }

    /// `GET` on v2, decoding the body without pagination metadata.
    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let (value, _) = self.get_v2(path, V2RequestOptions::default()).await?;
        Ok(value)
    }

    /// `GET` on v2 with cursor pagination — one page.
    pub(crate) async fn get_page<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Page<T>> {
        let path = Self::append_query(path, query);
        let (items, info) = self.get_v2_url(&path, page.to_options()).await?;
        Ok(Page {
            items,
            next_cursor: info.next_cursor,
        })
    }

    /// `GET` on v2 — follow the cursor until every page is consumed.
    pub(crate) async fn get_all<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<Vec<T>> {
        let mut all = Vec::new();
        let mut cursor = None;
        loop {
            let page = self
                .get_page(path, query, &PageParams { cursor, size: None })
                .await?;
            all.extend(page.items);
            match page.next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        Ok(all)
    }

    /// `PUT` on v2, returning the updated resource.
    pub(crate) async fn put_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.put_v2(path, body, V2RequestOptions::default()).await
    }

    /// `PUT` on v2 with an empty or void response body.
    pub(crate) async fn put<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<()> {
        self.put_v2_void(path, body, V2RequestOptions::default())
            .await
    }

    /// `POST` on v2 with an empty or void response body.
    pub(crate) async fn post_void<B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<()> {
        self.post_v2_void(path, body, V2RequestOptions::default())
            .await
    }

    /// `DELETE` on v2.
    pub(crate) async fn delete(&self, path: &str) -> Result<()> {
        self.delete_v2(path, V2RequestOptions::default()).await
    }

    /// `DELETE` on v2, decoding a JSON response body when the API returns one.
    pub(crate) async fn delete_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .send_request(
                ApiBranch::V2,
                Method::DELETE,
                path,
                None,
                V2RequestOptions::default(),
                true,
            )
            .await?;
        decode(resp).await
    }

    /// `POST` on v1 without consumer-key signing (e.g. `/auth/credential`).
    pub(crate) async fn post_v1_public<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let body = serde_json::to_string(body)?;
        let resp = self
            .send_request(
                ApiBranch::V1,
                Method::POST,
                path,
                Some(&body),
                V2RequestOptions::default(),
                false,
            )
            .await?;
        decode(resp).await
    }

    pub(crate) fn append_query(path: &str, query: &[(&str, &str)]) -> String {
        if query.is_empty() {
            return path.to_string();
        }
        let mut out = path.to_string();
        out.push('?');
        for (i, (k, v)) in query.iter().enumerate() {
            if i > 0 {
                out.push('&');
            }
            out.push_str(&urlencoding::encode(k));
            out.push('=');
            out.push_str(&urlencoding::encode(v));
        }
        out
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64
}

async fn consume(resp: reqwest::Response) -> Result<(StatusCode, String)> {
    let status = resp.status();
    let body = resp.text().await?;
    Ok((status, body))
}

fn page_info(resp: &reqwest::Response) -> V2PageInfo {
    V2PageInfo {
        next_cursor: resp
            .headers()
            .get("X-Pagination-Cursor-Next")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string),
    }
}

async fn decode<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
    let query_id = resp
        .headers()
        .get("X-Ovh-Queryid")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let status = resp.status();
    let body = resp.text().await?;
    if status.is_success() {
        if body.trim().is_empty() {
            return Err(OvhError::Api {
                status: status.as_u16(),
                code: Some("EMPTY_BODY".into()),
                message: "expected JSON response body".into(),
                query_id,
            });
        }
        Ok(serde_json::from_str(&body)?)
    } else {
        Err(api_error(status.as_u16(), &body, query_id))
    }
}

async fn decode_void(resp: reqwest::Response) -> Result<()> {
    let query_id = resp
        .headers()
        .get("X-Ovh-Queryid")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let status = resp.status();
    let body = resp.text().await?;
    if status.is_success() {
        Ok(())
    } else {
        Err(api_error(status.as_u16(), &body, query_id))
    }
}

fn api_error(status: u16, body: &str, query_id: Option<String>) -> OvhError {
    #[derive(serde::Deserialize)]
    struct ErrorBody {
        #[serde(rename = "errorCode")]
        error_code: Option<String>,
        class: Option<String>,
        message: Option<String>,
    }

    let parsed: Option<ErrorBody> = serde_json::from_str(body).ok();
    let code = parsed
        .as_ref()
        .and_then(|b| b.error_code.clone().or(b.class.clone()));
    let message = parsed
        .and_then(|b| b.message)
        .unwrap_or_else(|| body.trim().to_string());

    OvhError::Api {
        status,
        code,
        message,
        query_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_url_joins_v1_path() {
        let client = OvhClient::new(OvhEndpoint::OvhEu, "k", "s", "c");
        assert_eq!(
            client.api_url(ApiBranch::V1, "/allDom"),
            "https://eu.api.ovh.com/1.0/allDom"
        );
        assert_eq!(
            client.api_url(ApiBranch::V1, "allDom"),
            "https://eu.api.ovh.com/1.0/allDom"
        );
    }

    #[test]
    fn api_url_joins_v2_path() {
        let client = OvhClient::new(OvhEndpoint::OvhEu, "k", "s", "c");
        assert_eq!(
            client.api_url(ApiBranch::V2, "/domain/alldom"),
            "https://eu.api.ovh.com/v2/domain/alldom"
        );
    }

    #[test]
    fn append_query_joins_params() {
        let url = OvhClient::append_query("/domain/name", &[("searchValue", "a.b")]);
        assert_eq!(url, "/domain/name?searchValue=a.b");
    }

    #[test]
    fn endpoint_base_urls() {
        assert_eq!(
            OvhEndpoint::OvhUs.base_url_v1(),
            "https://api.us.ovhcloud.com/1.0"
        );
        assert_eq!(
            OvhEndpoint::OvhCa.base_url_v2(),
            "https://ca.api.ovh.com/v2"
        );
    }

    #[test]
    fn oauth2_token_urls() {
        assert_eq!(
            OvhEndpoint::OvhEu.oauth2_token_url(),
            "https://www.ovh.com/auth/oauth2/token"
        );
        assert_eq!(
            OvhEndpoint::OvhUs.oauth2_token_url(),
            "https://us.ovhcloud.com/auth/oauth2/token"
        );
        assert_eq!(
            OvhEndpoint::OvhCa.oauth2_token_url(),
            "https://ca.ovh.com/auth/oauth2/token"
        );
        assert_eq!(
            OvhEndpoint::KimsufiEu.oauth2_token_url(),
            "https://www.ovh.com/auth/oauth2/token"
        );
        assert_eq!(
            OvhEndpoint::SoyoustartCa.oauth2_token_url(),
            "https://ca.ovh.com/auth/oauth2/token"
        );
    }

    #[test]
    fn oauth2_client_construction() {
        let client = OvhClient::oauth2(OvhEndpoint::OvhEu, "my-id", "my-secret");
        assert!(matches!(client.auth, AuthMethod::OAuth2(_)));
        assert_eq!(client.endpoint(), OvhEndpoint::OvhEu);
    }

    #[test]
    fn from_auth_preserves_endpoint() {
        let client = OvhClient::from_auth(
            OvhEndpoint::OvhUs,
            AuthMethod::OAuth2(OAuth2Credentials::new("id", "secret")),
        );
        assert_eq!(client.endpoint(), OvhEndpoint::OvhUs);
    }
}
