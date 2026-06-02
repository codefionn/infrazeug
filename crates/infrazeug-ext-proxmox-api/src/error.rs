//! Error type for the Proxmox VE API client.

/// Result alias for fallible Proxmox API operations.
pub type Result<T> = std::result::Result<T, ProxmoxError>;

/// Anything that can go wrong while talking to the Proxmox VE API.
#[derive(Debug, thiserror::Error)]
pub enum ProxmoxError {
    /// The HTTP request could not be sent or the response could not be read.
    #[error("proxmox http transport error: {0}")]
    Http(#[from] reqwest::Error),

    /// The API answered with a non-2xx status.
    #[error("proxmox api error {status}: {message}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Human-readable error message.
        message: String,
    },

    /// A response body or request body could not be (de)serialized.
    #[error("proxmox json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Authentication (ticket acquisition or configuration) failed.
    #[error("proxmox auth error: {0}")]
    Auth(String),

    /// The client could not be constructed (e.g. TLS backend setup failed).
    #[error("proxmox client build error: {0}")]
    Build(String),

    /// A worker task (VM/container create, …) finished with a non-`OK` status.
    #[error("proxmox task {upid} failed: {exitstatus}")]
    Task {
        /// The task's unique process identifier.
        upid: String,
        /// The reported `exitstatus` (e.g. an error string).
        exitstatus: String,
    },

    /// A worker task did not finish within the configured timeout.
    #[error("proxmox task {upid} did not finish within {timeout_secs}s")]
    TaskTimeout {
        /// The task's unique process identifier.
        upid: String,
        /// The timeout that elapsed, in seconds.
        timeout_secs: u64,
    },
}
