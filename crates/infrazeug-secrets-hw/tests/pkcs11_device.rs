//! Live PKCS#11 token round-trip (SoftHSM/HSM/PIV). Ignored by default.
//!
//! Provision a SoftHSM token with an RSA key pair, then:
//!
//! ```sh
//! INFRZEUG_PKCS11_MODULE=/usr/lib/softhsm/libsofthsm2.so \
//! INFRZEUG_PKCS11_PIN=1234 \
//! INFRZEUG_PKCS11_LABEL=infrazeug \
//!   cargo test -p infrazeug-secrets-hw --features pkcs11-device \
//!   --test pkcs11_device -- --ignored
//! ```
#![cfg(feature = "pkcs11-device")]

use infrazeug_secrets::{create_envelope, generate_dek, unlock_envelope};
use infrazeug_secrets_hw::{Pkcs11Device, Pkcs11DeviceConfig};

fn config_from_env() -> Option<Pkcs11DeviceConfig> {
    let mut cfg = Pkcs11DeviceConfig::new(
        std::env::var("INFRZEUG_PKCS11_MODULE").ok()?,
        std::env::var("INFRZEUG_PKCS11_PIN").ok()?,
    );
    cfg.slot = std::env::var("INFRZEUG_PKCS11_SLOT")
        .ok()
        .and_then(|s| s.parse().ok());
    cfg.key_label = std::env::var("INFRZEUG_PKCS11_LABEL").ok();
    Some(cfg)
}

#[tokio::test]
#[ignore = "requires a PKCS#11 token via INFRZEUG_PKCS11_* env vars"]
async fn pkcs11_device_roundtrip() {
    let cfg = config_from_env().expect("INFRZEUG_PKCS11_MODULE + _PIN must be set");
    let provider = Pkcs11Device::new(cfg);

    let dek = generate_dek();
    let envelope = create_envelope("ops", &dek, &provider, "token-a")
        .await
        .expect("wrap on token");
    let entry = &envelope.recipients[0];
    let unlocked = unlock_envelope(&envelope, &provider, entry)
        .await
        .expect("unwrap on token");
    assert_eq!(unlocked.as_ref(), &dek);
}
