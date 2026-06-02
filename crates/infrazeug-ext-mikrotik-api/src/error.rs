//! Error type for the MikroTik RouterOS API client.

/// Result alias for fallible MikroTik API operations.
pub type Result<T> = std::result::Result<T, MikrotikError>;

/// Anything that can go wrong while talking to a RouterOS device.
#[derive(Debug, thiserror::Error)]
pub enum MikrotikError {
    /// The TCP/TLS connection could not be established or was reset.
    #[error("mikrotik transport error: {0}")]
    Transport(String),

    /// The wire protocol could not be encoded or decoded.
    #[error("mikrotik protocol error: {0}")]
    Protocol(String),

    /// Login failed or the router returned `!trap` / `!fatal`.
    #[error("mikrotik api error: {message}")]
    Api {
        /// RouterOS trap/fatal message (never includes credentials).
        message: String,
        /// Trap category when provided by the router.
        category: Option<u8>,
    },

    /// TLS setup failed (certificate, handshake, …).
    #[error("mikrotik tls error: {0}")]
    Tls(String),
}
