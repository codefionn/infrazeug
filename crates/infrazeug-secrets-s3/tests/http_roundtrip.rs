//! Live round-trip against a real S3-compatible server (MinIO/Ceph/AWS).
//!
//! Ignored by default; run with credentials in the environment:
//!
//! ```sh
//! INFRZEUG_S3_ENDPOINT=http://127.0.0.1:9000 \
//! INFRZEUG_S3_REGION=us-east-1 \
//! INFRZEUG_S3_BUCKET=vault \
//! INFRZEUG_S3_ACCESS_KEY=minioadmin \
//! INFRZEUG_S3_SECRET_KEY=minioadmin \
//!   cargo test -p infrazeug-secrets-s3 --test http_roundtrip -- --ignored
//! ```

use bytes::Bytes;
use infrazeug_secrets::backend::Backend;
use infrazeug_secrets_s3::{S3Config, S3HttpBackend};

fn config_from_env() -> Option<S3Config> {
    let mut cfg = S3Config::new(
        std::env::var("INFRZEUG_S3_ENDPOINT").ok()?,
        std::env::var("INFRZEUG_S3_REGION").unwrap_or_else(|_| "us-east-1".into()),
        std::env::var("INFRZEUG_S3_BUCKET").ok()?,
        std::env::var("INFRZEUG_S3_ACCESS_KEY").ok()?,
        std::env::var("INFRZEUG_S3_SECRET_KEY").ok()?,
    );
    cfg.session_token = std::env::var("INFRZEUG_S3_SESSION_TOKEN").ok();
    if std::env::var("INFRZEUG_S3_VHOST").is_ok() {
        cfg.path_style = false;
    }
    Some(cfg)
}

#[tokio::test]
#[ignore = "requires a live S3 endpoint via INFRZEUG_S3_* env vars"]
async fn put_get_list_delete_roundtrip() {
    let cfg = config_from_env().expect("INFRZEUG_S3_* env vars must be set");
    let be = S3HttpBackend::new(cfg).unwrap();

    let key = "infrazeug-test/roundtrip.bin";
    let body = Bytes::from_static(b"infrazeug s3 roundtrip payload");

    let meta = be.put(key, body.clone(), None).await.unwrap();
    assert_eq!(meta.size, body.len() as u64);

    let (got, _) = be
        .get(key)
        .await
        .unwrap()
        .expect("object present after put");
    assert_eq!(got, body);

    let listed = be.list("infrazeug-test/").await.unwrap();
    assert!(listed.iter().any(|m| m.key == key), "listed: {listed:?}");

    be.delete(key).await.unwrap();
    assert!(
        be.get(key).await.unwrap().is_none(),
        "object gone after delete"
    );
}
