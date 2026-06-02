//! Hardware-backed vault recipients: FIDO2 `hmac-secret` and PKCS#11 (SOUL §6.3).
//!
//! - [`Fido2Provider`] / [`Pkcs11Provider`] — deterministic fixtures for CI and
//!   unit tests (no physical token).
//! - `fido2-device` / `pkcs11-device` features — real CTAP2 and PKCS#11 modules
//!   ([`Fido2Device`], [`Pkcs11Device`]) with touch/PIN at unwrap time.
//!
//! Both families derive a KEK inside the token (HMAC-secret or RSA wrap) so the
//! DEK never leaves the envelope as cleartext.

mod fido2;
#[cfg(feature = "fido2-device")]
mod fido2_device;
mod pkcs11;
#[cfg(feature = "pkcs11-device")]
mod pkcs11_device;

pub use fido2::Fido2Provider;
#[cfg(feature = "fido2-device")]
pub use fido2_device::{Fido2Device, Fido2DeviceConfig};
pub use pkcs11::Pkcs11Provider;
#[cfg(feature = "pkcs11-device")]
pub use pkcs11_device::{Pkcs11Device, Pkcs11DeviceConfig};
