//! Production S3 `Backend` over HTTP with AWS Signature V4 (SOUL §6.5).
//!
//! Works against AWS S3 and S3-compatible servers (MinIO, Ceph RGW). Keys map
//! directly to object keys under `bucket`; optimistic concurrency uses the
//! server `ETag` via conditional `If-Match` writes, falling back to
//! last-write-wins on servers that ignore the precondition.

use crate::sigv4::{self, Credentials, SignRequest, EMPTY_PAYLOAD_SHA256};
use async_trait::async_trait;
use bytes::Bytes;
use infrazeug_secrets::backend::{validate_key, Backend, Etag, ObjectMeta};
use infrazeug_secrets::{Result, SecretsError};
use reqwest::{Client, Method, StatusCode};
use std::collections::BTreeMap;
use std::time::SystemTime;
use url::Url;

/// Connection + credential configuration for an S3 endpoint.
#[derive(Clone)]
pub struct S3Config {
    /// Base endpoint, e.g. `https://s3.us-east-1.amazonaws.com` or
    /// `http://127.0.0.1:9000` for MinIO.
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub session_token: Option<String>,
    /// Path-style (`endpoint/bucket/key`) vs virtual-hosted
    /// (`bucket.endpoint/key`). Path-style is the safe default for
    /// S3-compatible servers.
    pub path_style: bool,
}

impl S3Config {
    /// Path-style config from explicit credentials (MinIO/Ceph friendly).
    pub fn new(
        endpoint: impl Into<String>,
        region: impl Into<String>,
        bucket: impl Into<String>,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            region: region.into(),
            bucket: bucket.into(),
            access_key: access_key.into(),
            secret_key: secret_key.into(),
            session_token: None,
            path_style: true,
        }
    }
}

pub struct S3HttpBackend {
    client: Client,
    endpoint: Url,
    region: String,
    bucket: String,
    creds: Credentials,
    path_style: bool,
}

impl S3HttpBackend {
    pub fn new(cfg: S3Config) -> Result<Self> {
        let endpoint = Url::parse(&cfg.endpoint)
            .map_err(|e| SecretsError::Backend(format!("bad s3 endpoint: {e}")))?;
        Ok(Self {
            client: Client::new(),
            endpoint,
            region: cfg.region,
            bucket: cfg.bucket,
            creds: Credentials {
                access_key: cfg.access_key,
                secret_key: cfg.secret_key,
                session_token: cfg.session_token,
            },
            path_style: cfg.path_style,
        })
    }

    /// `Host` header value matching what reqwest will send (port only when
    /// non-default for the scheme).
    fn host_header(&self, host: &str) -> String {
        match self.endpoint.port() {
            Some(p) => format!("{host}:{p}"),
            None => host.to_string(),
        }
    }

    /// Resolve `(canonical_path, host_header, base_for_url)` for an object key.
    fn target(&self, key: &str) -> Result<(String, String)> {
        validate_key(key)?;
        let raw_host = self
            .endpoint
            .host_str()
            .ok_or_else(|| SecretsError::Backend("s3 endpoint has no host".into()))?
            .to_string();
        let key = key.trim_start_matches('/');
        if self.path_style {
            let path = format!("/{}/{}", self.bucket, key);
            Ok((path, self.host_header(&raw_host)))
        } else {
            let vhost = format!("{}.{}", self.bucket, raw_host);
            let path = format!("/{key}");
            Ok((path, self.host_header(&vhost)))
        }
    }

    /// Canonical path for bucket-scoped operations (e.g. ListObjectsV2).
    fn bucket_target(&self) -> Result<(String, String)> {
        let raw_host = self
            .endpoint
            .host_str()
            .ok_or_else(|| SecretsError::Backend("s3 endpoint has no host".into()))?
            .to_string();
        if self.path_style {
            Ok((format!("/{}", self.bucket), self.host_header(&raw_host)))
        } else {
            let vhost = format!("{}.{}", self.bucket, raw_host);
            Ok(("/".to_string(), self.host_header(&vhost)))
        }
    }

    /// Build the on-wire URL whose encoded path/query match the signed values.
    fn build_url(&self, host: &str, path: &str, query: &[(String, String)]) -> Result<Url> {
        let scheme = self.endpoint.scheme();
        let mut s = format!("{scheme}://{host}{}", sigv4::encode_path(path));
        if !query.is_empty() {
            s.push('?');
            s.push_str(&sigv4::encode_query(query));
        }
        Url::parse(&s).map_err(|e| SecretsError::Backend(format!("bad s3 url: {e}")))
    }

    async fn signed_send(
        &self,
        method: Method,
        path: &str,
        host: &str,
        query: &[(String, String)],
        body: Option<Bytes>,
        if_match: Option<&str>,
    ) -> Result<reqwest::Response> {
        let payload = body.as_deref().map(sigv4::payload_hash);
        let payload_hash = payload.as_deref().unwrap_or(EMPTY_PAYLOAD_SHA256);
        let (amz_date, date_stamp) = sigv4::now_timestamps();

        let mut headers = BTreeMap::new();
        headers.insert("host".to_string(), host.to_string());
        headers.insert("x-amz-content-sha256".to_string(), payload_hash.to_string());
        headers.insert("x-amz-date".to_string(), amz_date.clone());
        if let Some(tok) = &self.creds.session_token {
            headers.insert("x-amz-security-token".to_string(), tok.clone());
        }
        if let Some(tag) = if_match {
            headers.insert("if-match".to_string(), tag.to_string());
        }

        let auth = sigv4::authorization_header(
            &SignRequest {
                method: method.as_str(),
                path,
                query,
                headers: &headers,
                payload_sha256: payload_hash,
                region: &self.region,
                service: "s3",
            },
            &self.creds,
            &amz_date,
            &date_stamp,
        );

        let url = self.build_url(host, path, query)?;
        let mut req = self.client.request(method, url);
        for (k, v) in &headers {
            // reqwest sets `host` itself from the URL authority.
            if k != "host" {
                req = req.header(k.as_str(), v.as_str());
            }
        }
        req = req.header("Authorization", auth);
        if let Some(b) = body {
            req = req.body(b);
        }
        req.send()
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))
    }
}

fn etag_header(resp: &reqwest::Response) -> Option<Etag> {
    resp.headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|s| Etag(s.trim_matches('"').to_string()))
}

#[async_trait]
impl Backend for S3HttpBackend {
    async fn get(&self, key: &str) -> Result<Option<(Bytes, ObjectMeta)>> {
        let (path, host) = self.target(key)?;
        let resp = self
            .signed_send(Method::GET, &path, &host, &[], None, None)
            .await?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(SecretsError::Backend(format!(
                "s3 GET {key}: {}",
                resp.status()
            )));
        }
        let etag = etag_header(&resp);
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        let size = bytes.len() as u64;
        Ok(Some((
            bytes,
            ObjectMeta {
                key: key.to_string(),
                etag,
                mtime: Some(SystemTime::now()),
                size,
            },
        )))
    }

    async fn put(&self, key: &str, v: Bytes, prev: Option<&Etag>) -> Result<ObjectMeta> {
        let (path, host) = self.target(key)?;
        let size = v.len() as u64;
        let resp = self
            .signed_send(
                Method::PUT,
                &path,
                &host,
                &[],
                Some(v),
                prev.map(|e| e.0.as_str()),
            )
            .await?;
        if resp.status() == StatusCode::PRECONDITION_FAILED {
            return Err(SecretsError::Conflict {
                key: key.to_string(),
            });
        }
        if !resp.status().is_success() {
            return Err(SecretsError::Backend(format!(
                "s3 PUT {key}: {}",
                resp.status()
            )));
        }
        Ok(ObjectMeta {
            key: key.to_string(),
            etag: etag_header(&resp),
            mtime: Some(SystemTime::now()),
            size,
        })
    }

    async fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>> {
        validate_key(prefix.trim_end_matches('/'))?;
        let (path, host) = self.bucket_target()?;
        let query = vec![
            ("list-type".to_string(), "2".to_string()),
            ("prefix".to_string(), prefix.to_string()),
        ];
        let resp = self
            .signed_send(Method::GET, &path, &host, &query, None, None)
            .await?;
        if !resp.status().is_success() {
            return Err(SecretsError::Backend(format!(
                "s3 LIST {prefix}: {}",
                resp.status()
            )));
        }
        let text = resp
            .text()
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        parse_list_v2(&text)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let (path, host) = self.target(key)?;
        let resp = self
            .signed_send(Method::DELETE, &path, &host, &[], None, None)
            .await?;
        if resp.status() == StatusCode::NOT_FOUND || resp.status().is_success() {
            Ok(())
        } else {
            Err(SecretsError::Backend(format!(
                "s3 DELETE {key}: {}",
                resp.status()
            )))
        }
    }
}

/// Parse an S3 `ListObjectsV2` `ListBucketResult` into object metadata.
fn parse_list_v2(xml: &str) -> Result<Vec<ObjectMeta>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut buf = Vec::new();

    let mut in_contents = false;
    let mut cur_tag: Option<Vec<u8>> = None;
    let mut key = String::new();
    let mut etag: Option<Etag> = None;
    let mut size: u64 = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name().as_ref().to_vec();
                if name == b"Contents" {
                    in_contents = true;
                    key.clear();
                    etag = None;
                    size = 0;
                } else if in_contents {
                    cur_tag = Some(name);
                }
            }
            Ok(Event::Text(t)) if in_contents => {
                if let Some(tag) = &cur_tag {
                    let val = t.unescape().unwrap_or_default().into_owned();
                    match tag.as_slice() {
                        b"Key" => key = val,
                        b"ETag" => etag = Some(Etag(val.trim_matches('"').to_string())),
                        b"Size" => size = val.parse().unwrap_or(0),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"Contents" {
                    in_contents = false;
                    if !key.is_empty() {
                        out.push(ObjectMeta {
                            key: std::mem::take(&mut key),
                            etag: etag.take(),
                            mtime: Some(SystemTime::now()),
                            size,
                        });
                    }
                }
                cur_tag = None;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(SecretsError::Backend(format!("s3 list parse: {e}"))),
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_list_v2_extracts_objects() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Name>vault</Name>
  <Prefix>data-keys/</Prefix>
  <KeyCount>2</KeyCount>
  <Contents>
    <Key>data-keys/prod.cbor</Key>
    <LastModified>2024-01-01T00:00:00.000Z</LastModified>
    <ETag>&quot;abc123&quot;</ETag>
    <Size>128</Size>
  </Contents>
  <Contents>
    <Key>data-keys/stage.cbor</Key>
    <ETag>"def456"</ETag>
    <Size>64</Size>
  </Contents>
</ListBucketResult>"#;
        let items = parse_list_v2(xml).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].key, "data-keys/prod.cbor");
        assert_eq!(items[0].etag.as_ref().unwrap().0, "abc123");
        assert_eq!(items[0].size, 128);
        assert_eq!(items[1].key, "data-keys/stage.cbor");
        assert_eq!(items[1].etag.as_ref().unwrap().0, "def456");
    }

    #[test]
    fn parse_list_v2_empty() {
        let xml =
            r#"<ListBucketResult><Name>vault</Name><KeyCount>0</KeyCount></ListBucketResult>"#;
        assert!(parse_list_v2(xml).unwrap().is_empty());
    }

    #[test]
    fn path_style_target() {
        let be = S3HttpBackend::new(S3Config::new(
            "http://127.0.0.1:9000",
            "us-east-1",
            "vault",
            "ak",
            "sk",
        ))
        .unwrap();
        let (path, host) = be.target("data-keys/prod.cbor").unwrap();
        assert_eq!(path, "/vault/data-keys/prod.cbor");
        assert_eq!(host, "127.0.0.1:9000");
        let url = be.build_url(&host, &path, &[]).unwrap();
        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:9000/vault/data-keys/prod.cbor"
        );
    }

    #[test]
    fn virtual_hosted_target() {
        let mut cfg = S3Config::new(
            "https://s3.us-east-1.amazonaws.com",
            "us-east-1",
            "examplebucket",
            "ak",
            "sk",
        );
        cfg.path_style = false;
        let be = S3HttpBackend::new(cfg).unwrap();
        let (path, host) = be.target("test.txt").unwrap();
        assert_eq!(path, "/test.txt");
        assert_eq!(host, "examplebucket.s3.us-east-1.amazonaws.com");
    }
}
