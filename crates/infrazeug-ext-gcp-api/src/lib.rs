//! GCP API client for infrazeug.
//!
//! Service-account authenticated REST client for Compute Engine, Cloud Storage,
//! and IAM — dependency-light like [`infrazeug_ext_ionos_cloud_api`].

pub mod auth;
mod client;
pub mod compute;
pub mod error;
pub mod iam;
pub mod storage;

pub use auth::{GcpAuth, ServiceAccountKey};
pub use client::{GcpClient, GcpConfig};
pub use error::{GcpError, Result};
