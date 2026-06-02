//! WebDAV `Backend` via PROPFIND / GET / PUT / DELETE.

use async_trait::async_trait;
use base64::Engine;
use bytes::Bytes;
use infrazeug_secrets::backend::{validate_key, Backend, Etag, ObjectMeta};
use infrazeug_secrets::{Result, SecretsError};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, IF_MATCH};
use reqwest::{Client, Method, StatusCode};
use sha2::{Digest, Sha256};
use std::time::SystemTime;
use url::Url;

pub struct WebDavBackend {
    client: Client,
    base: Url,
    auth: Option<String>,
}

impl WebDavBackend {
    pub fn new(
        base_url: impl AsRef<str>,
        user: Option<&str>,
        password: Option<&str>,
    ) -> Result<Self> {
        let mut base = Url::parse(base_url.as_ref())
            .map_err(|e| SecretsError::Backend(format!("bad webdav url: {e}")))?;
        if !base.path().ends_with('/') {
            base = base
                .join("")
                .map_err(|e| SecretsError::Backend(format!("bad webdav url: {e}")))?;
        }
        let auth = match (user, password) {
            (Some(u), Some(p)) => {
                let token = base64::engine::general_purpose::STANDARD.encode(format!("{u}:{p}"));
                Some(format!("Basic {token}"))
            }
            _ => None,
        };
        Ok(Self {
            client: Client::new(),
            base,
            auth,
        })
    }

    fn url_for(&self, key: &str) -> Result<Url> {
        validate_key(key)?;
        let url = self
            .base
            .join(key.trim_start_matches('/'))
            .map_err(|e| SecretsError::Backend(format!("bad webdav key: {e}")))?;
        if url.as_str().starts_with(self.base.as_str()) {
            Ok(url)
        } else {
            Err(SecretsError::Backend(format!(
                "webdav key {key:?} escapes base URL"
            )))
        }
    }

    async fn request(
        &self,
        method: Method,
        url: Url,
        body: Option<Bytes>,
        if_match: Option<&str>,
    ) -> Result<reqwest::Response> {
        let mut req = self.client.request(method, url);
        if let Some(a) = &self.auth {
            req = req.header(AUTHORIZATION, a);
        }
        if let Some(tag) = if_match {
            req = req.header(IF_MATCH, tag);
        }
        if let Some(b) = body {
            req = req.header(CONTENT_TYPE, "application/octet-stream").body(b);
        }
        req.send()
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))
    }

    fn meta_from(key: &str, etag: Option<String>, len: u64) -> ObjectMeta {
        ObjectMeta {
            key: key.to_string(),
            etag: etag.map(Etag),
            mtime: Some(SystemTime::now()),
            size: len,
        }
    }
}

#[async_trait]
impl Backend for WebDavBackend {
    async fn get(&self, key: &str) -> Result<Option<(Bytes, ObjectMeta)>> {
        let url = self.url_for(key)?;
        let resp = self.request(Method::GET, url, None, None).await?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(SecretsError::Backend(format!(
                "webdav GET {key}: {}",
                resp.status()
            )));
        }
        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim_matches('"').to_string());
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        let len = bytes.len() as u64;
        let etag_hash = Etag(hex::encode(Sha256::digest(&bytes)));
        Ok(Some((
            bytes,
            ObjectMeta {
                key: key.to_string(),
                etag: etag.map(Etag).or(Some(etag_hash)),
                mtime: Some(SystemTime::now()),
                size: len,
            },
        )))
    }

    async fn put(&self, key: &str, v: Bytes, prev: Option<&Etag>) -> Result<ObjectMeta> {
        let url = self.url_for(key)?;
        let if_match = prev.map(|e| e.0.as_str());
        let resp = self
            .request(Method::PUT, url.clone(), Some(v.clone()), if_match)
            .await?;
        if resp.status() == StatusCode::PRECONDITION_FAILED {
            return Err(SecretsError::Conflict {
                key: key.to_string(),
            });
        }
        if !resp.status().is_success() {
            return Err(SecretsError::Backend(format!(
                "webdav PUT {key}: {}",
                resp.status()
            )));
        }
        Ok(Self::meta_from(
            key,
            resp.headers()
                .get("etag")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.trim_matches('"').to_string()),
            v.len() as u64,
        ))
    }

    async fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>> {
        validate_key(prefix.trim_end_matches('/'))?;
        let url = self.url_for(prefix.trim_end_matches('/'))?;
        let body = concat!(
            r#"<?xml version="1.0" encoding="utf-8"?>"#,
            r#"<propfind xmlns="DAV:"><prop><getcontentlength/><getetag/></prop></propfind>"#
        );
        let mut req = self
            .client
            .request(Method::from_bytes(b"PROPFIND").unwrap(), url);
        if let Some(a) = &self.auth {
            req = req.header(AUTHORIZATION, a);
        }
        req = req
            .header("Depth", "infinity")
            .header(CONTENT_TYPE, "application/xml")
            .body(body);
        let resp = req
            .send()
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(SecretsError::Backend(format!(
                "webdav PROPFIND {prefix}: {}",
                resp.status()
            )));
        }
        let text = resp
            .text()
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        parse_propfind(prefix, &text)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let url = self.url_for(key)?;
        let resp = self.request(Method::DELETE, url, None, None).await?;
        if resp.status() == StatusCode::NOT_FOUND || resp.status().is_success() {
            Ok(())
        } else {
            Err(SecretsError::Backend(format!(
                "webdav DELETE {key}: {}",
                resp.status()
            )))
        }
    }
}

fn parse_propfind(prefix: &str, xml: &str) -> Result<Vec<ObjectMeta>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut current_href: Option<String> = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"response" => {
                current_href = None;
            }
            Ok(Event::Start(e)) if e.name().as_ref() == b"href" => {
                if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                    current_href = Some(t.unescape().unwrap_or_default().into_owned());
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"response" => {
                if let Some(href) = current_href.take() {
                    let key = href_to_key(&href, prefix);
                    if key.starts_with(prefix) && !key.ends_with('/') {
                        out.push(ObjectMeta {
                            key,
                            etag: None,
                            mtime: Some(SystemTime::now()),
                            size: 0,
                        });
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(SecretsError::Backend(format!("propfind parse: {e}")));
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

fn href_to_key(href: &str, _prefix: &str) -> String {
    let path = href.trim_start_matches('/');
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_propfind() {
        let xml = r#"<?xml version="1.0"?><multistatus xmlns="DAV:"></multistatus>"#;
        let items = parse_propfind("files/", xml).unwrap();
        assert!(items.is_empty());
    }
}
