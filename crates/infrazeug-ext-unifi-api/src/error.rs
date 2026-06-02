//! Error type for the UniFi Network controller API client.

/// Result alias for fallible UniFi API operations.
pub type Result<T> = std::result::Result<T, UnifiError>;

/// Anything that can go wrong while talking to the UniFi Network controller.
#[derive(Debug, thiserror::Error)]
pub enum UnifiError {
    /// The HTTP request could not be sent or the response could not be read.
    #[error("unifi http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// The controller answered with a non-`ok` envelope or a non-2xx status. The
    /// `message` carries the UniFi `meta.msg` code (e.g. `api.err.NoSiteContext`)
    /// when present, otherwise the raw body.
    #[error("unifi api error {status}: {message}")]
    Api {
        /// HTTP status code (or `200` when only the envelope reported the error).
        status: u16,
        /// Human-readable error message / UniFi error code.
        message: String,
    },

    /// A response or request body could not be (de)serialized.
    #[error("unifi json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Login / session establishment failed.
    #[error("unifi auth error: {0}")]
    Auth(String),
}
