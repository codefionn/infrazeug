//! AWS Signature Version 4 request signing (SOUL §6.5 — production S3 backend).
//!
//! Pure, side-effect-free signing helpers so the canonical request and the
//! final `Authorization` header can be unit-tested against AWS's published
//! test vectors with a fixed timestamp.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

/// Static long-term S3 credentials (optionally with an STS session token).
#[derive(Clone)]
pub struct Credentials {
    pub access_key: String,
    pub secret_key: String,
    pub session_token: Option<String>,
}

/// SHA-256 of an empty payload — used for bodyless requests.
pub const EMPTY_PAYLOAD_SHA256: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// Hex-encoded SHA-256 of `data`, suitable for the `x-amz-content-sha256` header.
pub fn payload_hash(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn hmac(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

/// RFC 3986 percent-encoding. `encode_slash = false` preserves `/` for path
/// components; query keys/values encode everything except the unreserved set.
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

/// Inputs for a single signed request. `headers` keys must already be
/// lowercase and must include `host` plus the `x-amz-*` headers being signed.
pub struct SignRequest<'a> {
    pub method: &'a str,
    /// Path component, not yet percent-encoded (e.g. `/bucket/my key.txt`).
    pub path: &'a str,
    /// Unencoded query pairs; signing sorts and encodes them.
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

/// Percent-encode a request path exactly as the signer does, so the on-wire
/// URL matches the signed canonical URI.
pub fn encode_path(path: &str) -> String {
    canonical_path(path)
}

/// Build the canonical (sorted, percent-encoded) query string used for both
/// signing and the on-wire URL.
pub fn encode_query(query: &[(String, String)]) -> String {
    canonical_query(query)
}

/// Build the canonical request string and the `;`-joined signed-header list.
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

/// Compute the `Authorization` header value for `req` at the given timestamp.
///
/// `amz_date` is `YYYYMMDDTHHMMSSZ`; `date_stamp` is `YYYYMMDD`. Both must
/// match the `x-amz-date` header in `req.headers`.
pub fn authorization_header(
    req: &SignRequest,
    creds: &Credentials,
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
        format!("AWS4{}", creds.secret_key).as_bytes(),
        date_stamp.as_bytes(),
    );
    let k_region = hmac(&k_date, req.region.as_bytes());
    let k_service = hmac(&k_region, req.service.as_bytes());
    let k_signing = hmac(&k_service, b"aws4_request");
    let signature = hex::encode(hmac(&k_signing, string_to_sign.as_bytes()));

    format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
        creds.access_key,
    )
}

/// Current UTC time formatted as (`YYYYMMDDTHHMMSSZ`, `YYYYMMDD`).
pub fn now_timestamps() -> (String, String) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_timestamps(secs)
}

/// Format a Unix timestamp (seconds) as (`amz_date`, `date_stamp`), UTC.
///
/// Uses Howard Hinnant's civil-from-days algorithm to avoid a calendar
/// dependency.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// AWS-documented "GET Object" example for SigV4 (S3 dev guide).
    /// Validates canonical request + signing-key chain against a known answer.
    #[test]
    fn aws_get_object_vector() {
        let creds = Credentials {
            access_key: "AKIAIOSFODNN7EXAMPLE".into(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into(),
            session_token: None,
        };
        let mut headers = BTreeMap::new();
        headers.insert("host".into(), "examplebucket.s3.amazonaws.com".into());
        headers.insert("range".into(), "bytes=0-9".into());
        headers.insert("x-amz-content-sha256".into(), EMPTY_PAYLOAD_SHA256.into());
        headers.insert("x-amz-date".into(), "20130524T000000Z".into());

        let req = SignRequest {
            method: "GET",
            path: "/test.txt",
            query: &[],
            headers: &headers,
            payload_sha256: EMPTY_PAYLOAD_SHA256,
            region: "us-east-1",
            service: "s3",
        };

        let auth = authorization_header(&req, &creds, "20130524T000000Z", "20130524");
        assert!(
            auth.ends_with(
                "Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
            ),
            "got: {auth}"
        );
        assert!(auth.contains("SignedHeaders=host;range;x-amz-content-sha256;x-amz-date"));
    }

    #[test]
    fn timestamp_formatting() {
        // 2013-05-24T00:00:00Z == 1369353600
        assert_eq!(
            format_timestamps(1_369_353_600),
            ("20130524T000000Z".into(), "20130524".into())
        );
        // 2023-11-15T13:45:30Z == 1700055930
        assert_eq!(
            format_timestamps(1_700_055_930),
            ("20231115T134530Z".into(), "20231115".into())
        );
    }

    #[test]
    fn uri_encode_preserves_slash_in_path() {
        assert_eq!(uri_encode("/bucket/a b.txt", false), "/bucket/a%20b.txt");
        assert_eq!(uri_encode("a/b", true), "a%2Fb");
    }
}
