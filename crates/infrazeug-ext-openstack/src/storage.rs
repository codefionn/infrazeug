//! S3 bucket create/exists via AWS Signature V4 (OVH High Performance Object Storage).

use crate::error::{OpenstackError, Result};
use infrazeug_secrets_s3::sigv4::{
    self, authorization_header, encode_path, encode_query, Credentials, SignRequest,
    EMPTY_PAYLOAD_SHA256,
};
use reqwest::{Client, Method, StatusCode};
use std::collections::BTreeMap;
use url::Url;

/// Default OVH Public Cloud S3 endpoint for a region (e.g. `DE` → `https://s3.de.io.cloud.ovh.net`).
pub fn s3_endpoint(region: &str) -> String {
    format!(
        "https://s3.{}.io.cloud.ovh.net",
        region.trim().to_ascii_lowercase()
    )
}

fn host_header(endpoint: &Url, host: &str) -> String {
    match endpoint.port() {
        Some(p) => format!("{host}:{p}"),
        None => host.to_string(),
    }
}

async fn signed_request(
    http: &Client,
    method: Method,
    endpoint: &str,
    region: &str,
    creds: &Credentials,
    bucket: &str,
    body: Option<&[u8]>,
) -> Result<reqwest::Response> {
    let endpoint_url = Url::parse(endpoint)
        .map_err(|e| OpenstackError::Url(format!("bad s3 endpoint {endpoint}: {e}")))?;
    let host = endpoint_url
        .host_str()
        .ok_or_else(|| OpenstackError::Url("s3 endpoint has no host".into()))?;
    let path = format!("/{bucket}");
    let host_hdr = host_header(&endpoint_url, host);
    let query: &[(String, String)] = &[];
    let payload_hash = body
        .map(sigv4::payload_hash)
        .unwrap_or_else(|| EMPTY_PAYLOAD_SHA256.to_string());
    let (amz_date, date_stamp) = sigv4::now_timestamps();

    let mut headers = BTreeMap::new();
    headers.insert("host".to_string(), host_hdr.clone());
    headers.insert("x-amz-content-sha256".to_string(), payload_hash.clone());
    headers.insert("x-amz-date".to_string(), amz_date.clone());
    if let Some(tok) = &creds.session_token {
        headers.insert("x-amz-security-token".to_string(), tok.clone());
    }

    let auth = authorization_header(
        &SignRequest {
            method: method.as_str(),
            path: &path,
            query,
            headers: &headers,
            payload_sha256: &payload_hash,
            region: &region.to_ascii_lowercase(),
            service: "s3",
        },
        creds,
        &amz_date,
        &date_stamp,
    );

    let mut url = format!(
        "{}://{}{}",
        endpoint_url.scheme(),
        host_hdr,
        encode_path(&path)
    );
    let encoded_query = encode_query(query);
    if !encoded_query.is_empty() {
        url.push('?');
        url.push_str(&encoded_query);
    }
    let url = Url::parse(&url).map_err(|e| OpenstackError::Url(e.to_string()))?;

    let mut req = http.request(method, url);
    for (k, v) in &headers {
        if k != "host" {
            req = req.header(k.as_str(), v.as_str());
        }
    }
    req = req.header("Authorization", auth);
    if let Some(b) = body {
        req = req.body(b.to_vec());
    }
    Ok(req.send().await?)
}

/// `HEAD /{bucket}` — returns `true` when the bucket exists (HTTP 200).
pub async fn bucket_exists(
    endpoint: &str,
    region: &str,
    creds: &Credentials,
    bucket: &str,
) -> Result<bool> {
    let http = Client::new();
    let resp = signed_request(&http, Method::HEAD, endpoint, region, creds, bucket, None).await?;
    match resp.status() {
        StatusCode::OK => Ok(true),
        StatusCode::NOT_FOUND => Ok(false),
        other => {
            let body = resp.text().await.unwrap_or_default();
            Err(OpenstackError::Api {
                status: other.as_u16(),
                message: body,
            })
        }
    }
}

/// `PUT /{bucket}` — create the bucket. Treats 200/204 and 409 owned as success.
pub async fn create_bucket(
    endpoint: &str,
    region: &str,
    creds: &Credentials,
    bucket: &str,
) -> Result<()> {
    let http = Client::new();
    let resp = signed_request(&http, Method::PUT, endpoint, region, creds, bucket, None).await?;
    let status = resp.status();
    if status.is_success() || status == StatusCode::CONFLICT {
        return Ok(());
    }
    let body = resp.text().await.unwrap_or_default();
    if body.contains("BucketAlreadyOwnedByYou") {
        return Ok(());
    }
    Err(OpenstackError::Api {
        status: status.as_u16(),
        message: body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s3_endpoint_formats_region() {
        assert_eq!(s3_endpoint("DE"), "https://s3.de.io.cloud.ovh.net");
        assert_eq!(s3_endpoint("gra"), "https://s3.gra.io.cloud.ovh.net");
    }
}
