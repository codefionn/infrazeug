//! Error type for the OpenStack client.

/// Result alias for fallible OpenStack operations.
pub type Result<T> = std::result::Result<T, OpenstackError>;

/// Anything that can go wrong while talking to OpenStack (Keystone or S3).
#[derive(Debug, thiserror::Error)]
pub enum OpenstackError {
    /// The HTTP request could not be sent or the response could not be read.
    #[error("openstack http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// The API answered with a non-success status.
    #[error("openstack api error {status}: {message}")]
    Api { status: u16, message: String },

    /// A response body could not be (de)serialized.
    #[error("openstack json error: {0}")]
    Json(#[from] serde_json::Error),

    /// The configured endpoint or a built request URL was not valid.
    #[error("invalid openstack url: {0}")]
    Url(String),

    /// Authentication is required before this operation.
    #[error("openstack client is not authenticated")]
    NotAuthenticated,

    /// A required service was missing from the Keystone catalog.
    #[error("openstack catalog missing service type {service_type} in region {region}")]
    CatalogMissing {
        service_type: String,
        region: String,
    },
}
