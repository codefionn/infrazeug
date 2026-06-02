//! OpenStack (Keystone + S3) client for infrazeug.
//!
//! Low-level HTTP bindings for OVH Public Cloud OpenStack credentials:
//! Keystone v3 password auth, EC2/S3 credential management, and S3 bucket
//! operations via AWS Signature V4.

mod auth;
mod client;
mod error;
mod identity;
mod storage;

pub use auth::{catalog_endpoint, OpenstackConfig};
pub use client::OpenstackClient;
pub use error::{OpenstackError, Result};
pub use identity::Ec2Credential;
pub use storage::{bucket_exists, create_bucket, s3_endpoint};
