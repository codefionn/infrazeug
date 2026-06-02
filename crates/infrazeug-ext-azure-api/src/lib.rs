//! Azure API client for infrazeug.
//!
//! OAuth2 client-credentials authenticated REST client for ARM Compute, Blob
//! Storage, and storage-account keys.

pub mod auth;
mod client;
pub mod compute;
pub mod error;
pub mod storage;

pub use auth::{AzureAuth, AzureCredentials};
pub use client::{AzureClient, AzureConfig};
pub use error::{AzureError, Result};
