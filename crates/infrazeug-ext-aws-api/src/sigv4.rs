//! AWS Signature Version 4 request signing.

use crate::auth::AwsCredentials;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

pub const EMPTY_PAYLOAD_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

pub fn payload_hash(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn hmac(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

fn uri_encode(s: &str, encode_slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            b'/' if !encode_slash => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub struct SignRequest<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub query: &'a [(String, String)],
    pub headers: &'a BTreeMap<String, String>,
    pub payload_sha256: &'a str,
    pub region: &'a str,
    pub service: &'a str,
}

fn canonical_query(query: &[(String, String)]) -> String {
    let mut pairs: Vec<(String, String)> = query
        .iter()
        .map(|(k, v)| (uri_encode(k, true), uri_encode(v, true)))
        .collect();
    pairs.sort();
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn canonical_path(path: &str) -> String {
    if path.is_empty() {
        "/".to_string()
    } else {
        uri_encode(path, false)
    }
}

pub fn encode_path(path: &str) -> String {
    canonical_path(path)
}

pub fn encode_query(query: &[(String, String)]) -> String {
    canonical_query(query)
}

fn canonical_request(req: &SignRequest) -> (String, String) {
    let signed_headers = req.headers.keys().cloned().collect::<Vec<_>>().join(";");
    let canonical_headers = req
        .headers
        .iter()
        .map(|(k, v)| format!("{k}:{}\n", v.trim()))
        .collect::<String>();
    let canonical = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        req.method,
        canonical_path(req.path),
        canonical_query(req.query),
        canonical_headers,
        signed_headers,
        req.payload_sha256,
    );
    (canonical, signed_headers)
}

pub fn authorization_header(
    req: &SignRequest,
    creds: &AwsCredentials,
    amz_date: &str,
    date_stamp: &str,
) -> String {
    let (canonical, signed_headers) = canonical_request(req);
    let scope = format!("{date_stamp}/{}/{}/aws4_request", req.region, req.service);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        hex::encode(Sha256::digest(canonical.as_bytes()))
    );

    let k_date = hmac(
        format!("AWS4{}", creds.secret_access_key).as_bytes(),
        date_stamp.as_bytes(),
    );
    let k_region = hmac(&k_date, req.region.as_bytes());
    let k_service = hmac(&k_region, req.service.as_bytes());
    let k_signing = hmac(&k_service, b"aws4_request");
    let signature = hex::encode(hmac(&k_signing, string_to_sign.as_bytes()));

    format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
        creds.access_key_id,
    )
}

pub fn now_timestamps() -> (String, String) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_timestamps(secs)
}

pub fn format_timestamps(unix_secs: u64) -> (String, String) {
    let days = (unix_secs / 86_400) as i64;
    let rem = unix_secs % 86_400;
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }

    let amz_date = format!("{year:04}{month:02}{day:02}T{hh:02}{mm:02}{ss:02}Z");
    let date_stamp = format!("{year:04}{month:02}{day:02}");
    (amz_date, date_stamp)
}
