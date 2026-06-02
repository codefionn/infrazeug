//! Error type for the OVHcloud API client.

/// Result alias for fallible OVH API operations.
pub type Result<T> = std::result::Result<T, OvhError>;

/// Anything that can go wrong while talking to the OVHcloud API.
#[derive(Debug, thiserror::Error)]
pub enum OvhError {
    /// The HTTP request could not be sent or the response could not be read
    /// (DNS, TLS, connection reset, timeout, …).
    #[error("ovh http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// The API answered with a non-2xx status. `message` is the OVH error
    /// message when the body was a structured error, otherwise the raw body.
    #[error("ovh api error {status}: {message}")]
    Api {
        /// HTTP status code (or `0` for client-side protocol problems such as a
        /// malformed `/auth/time` response).
        status: u16,
        /// OVH `errorCode`/`class` discriminator, when present.
        code: Option<String>,
        /// Human-readable error message.
        message: String,
        /// Value of the `X-Ovh-Queryid` response header, useful for support.
        query_id: Option<String>,
    },

    /// A response body (or a request body) could not be (de)serialized to the
    /// expected JSON shape.
    #[error("ovh json error: {0}")]
    Json(#[from] serde_json::Error),

    /// The configured endpoint or a built request URL was not a valid URL.
    #[error("invalid ovh url: {0}")]
    Url(String),
}
