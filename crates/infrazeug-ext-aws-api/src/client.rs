//! SigV4-signed HTTP client for AWS service endpoints.

use crate::auth::AwsCredentials;
use crate::error::{AwsError, Result};
use crate::sigv4::{
    authorization_header, encode_path, encode_query, now_timestamps, payload_hash, SignRequest,
    EMPTY_PAYLOAD_SHA256,
};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::{Client, Method, StatusCode};
use std::collections::BTreeMap;

/// Connection configuration for an AWS API client.
#[derive(Debug, Clone)]
pub struct AwsConfig {
    pub credentials: AwsCredentials,
    /// Default region for EC2 and regional S3 (e.g. `us-east-1`).
    pub region: String,
}

impl AwsConfig {
    pub fn new(credentials: AwsCredentials, region: impl Into<String>) -> Self {
        Self {
            credentials,
            region: region.into(),
        }
    }
}

/// An authenticated AWS API client (EC2 Query API, S3 REST, IAM Query API).
#[derive(Clone)]
pub struct AwsClient {
    http: Client,
    config: AwsConfig,
}

impl AwsClient {
    pub fn new(config: AwsConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    pub fn config(&self) -> &AwsConfig {
        &self.config
    }

    pub(crate) fn region(&self) -> &str {
        &self.config.region
    }

    pub(crate) fn ec2_host(&self) -> String {
        format!("ec2.{}.amazonaws.com", self.config.region)
    }

    pub(crate) fn s3_host(&self, bucket: Option<&str>) -> String {
        match bucket {
            Some(name) => format!("{name}.s3.{}.amazonaws.com", self.config.region),
            None => format!("s3.{}.amazonaws.com", self.config.region),
        }
    }

    pub(crate) fn iam_host(&self) -> &'static str {
        "iam.amazonaws.com"
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn signed_request(
        &self,
        method: Method,
        host: &str,
        path: &str,
        query: &[(String, String)],
        body: Option<&[u8]>,
        service: &str,
        content_type: Option<&str>,
    ) -> Result<reqwest::Response> {
        let payload = body.unwrap_or_default();
        let payload_sha256 = if payload.is_empty() {
            EMPTY_PAYLOAD_SHA256.to_string()
        } else {
            payload_hash(payload)
        };

        let (amz_date, date_stamp) = now_timestamps();
        let mut headers = BTreeMap::new();
        headers.insert("host".into(), host.to_string());
        headers.insert("x-amz-date".into(), amz_date.clone());
        headers.insert("x-amz-content-sha256".into(), payload_sha256.clone());
        if let Some(token) = &self.config.credentials.session_token {
            headers.insert("x-amz-security-token".into(), token.clone());
        }
        if let Some(ct) = content_type {
            headers.insert("content-type".into(), ct.to_string());
        }

        let sign_req = SignRequest {
            method: method.as_str(),
            path,
            query,
            headers: &headers,
            payload_sha256: &payload_sha256,
            region: &self.config.region,
            service,
        };
        let auth =
            authorization_header(&sign_req, &self.config.credentials, &amz_date, &date_stamp);

        let mut url = format!("https://{host}{}", encode_path(path));
        let qs = encode_query(query);
        if !qs.is_empty() {
            url.push('?');
            url.push_str(&qs);
        }

        let mut header_map = HeaderMap::new();
        for (k, v) in &headers {
            let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| AwsError::Auth(e.to_string()))?;
            header_map.insert(
                name,
                HeaderValue::from_str(v).map_err(|e| AwsError::Auth(e.to_string()))?,
            );
        }
        header_map.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth).map_err(|e| AwsError::Auth(e.to_string()))?,
        );

        let mut req = self.http.request(method, &url).headers(header_map);
        if !payload.is_empty() {
            req = req.body(payload.to_vec());
        }
        Ok(req.send().await?)
    }

    pub(crate) async fn ec2_query(
        &self,
        params: &[(String, String)],
    ) -> Result<(StatusCode, String)> {
        let body = encode_query(params);
        let resp = self
            .signed_request(
                Method::POST,
                &self.ec2_host(),
                "/",
                &[],
                Some(body.as_bytes()),
                "ec2",
                Some("application/x-www-form-urlencoded; charset=utf-8"),
            )
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        Ok((status, text))
    }

    pub(crate) async fn iam_query(
        &self,
        params: &[(String, String)],
    ) -> Result<(StatusCode, String)> {
        let body = encode_query(params);
        let resp = self
            .signed_request(
                Method::POST,
                self.iam_host(),
                "/",
                &[],
                Some(body.as_bytes()),
                "iam",
                Some("application/x-www-form-urlencoded; charset=utf-8"),
            )
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        Ok((status, text))
    }

    pub(crate) async fn s3_request(
        &self,
        method: Method,
        bucket: Option<&str>,
        path: &str,
        query: &[(String, String)],
        body: Option<&[u8]>,
    ) -> Result<(StatusCode, String)> {
        let host = self.s3_host(bucket);
        let resp = self
            .signed_request(
                method,
                &host,
                path,
                query,
                body,
                "s3",
                body.map(|_| "application/xml"),
            )
            .await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Ok((status, text))
    }
}

pub(crate) fn api_error(status: StatusCode, body: &str) -> AwsError {
    AwsError::Api {
        status: status.as_u16(),
        message: body.to_string(),
    }
}

pub(crate) fn ensure_success(status: StatusCode, body: &str) -> Result<()> {
    if status.is_success() {
        Ok(())
    } else {
        Err(api_error(status, body))
    }
}
