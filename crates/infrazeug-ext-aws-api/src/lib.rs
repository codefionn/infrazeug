//! AWS API client for infrazeug.
//!
//! A typed, async client for EC2 (compute + EBS), S3, and IAM using SigV4-signed
//! HTTP requests (no heavyweight SDK). Mirrors the dependency-light style of
//! [`infrazeug_ext_ionos_cloud_api`] and [`infrazeug_ext_ovh_api`].
//!
//! # Surfaces
//!
//! - [`ec2`] — `RunInstances`, `DescribeInstances`, `CreateVolume`, `DescribeVolumes`
//! - [`s3`] — bucket list/create/exists via REST
//! - [`iam`] — user + access-key lifecycle
//!
//! # Example
//!
//! ```no_run
//! use infrazeug_ext_aws_api::{AwsClient, AwsConfig, AwsCredentials};
//! use infrazeug_ext_aws_api::ec2::InstanceCreate;
//!
//! # async fn run() -> infrazeug_ext_aws_api::Result<()> {
//! let client = AwsClient::new(AwsConfig::new(
//!     AwsCredentials::new(
//!         std::env::var("AWS_ACCESS_KEY_ID").unwrap(),
//!         std::env::var("AWS_SECRET_ACCESS_KEY").unwrap(),
//!     ),
//!     std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".into()),
//! ));
//!
//! let instances = client.ec2_instances("web-1").await?;
//! println!("found {} instances", instances.len());
//! # Ok(())
//! # }
//! ```

pub mod auth;
mod client;
pub mod ec2;
pub mod error;
pub mod iam;
pub mod s3;
mod sigv4;

pub use auth::AwsCredentials;
pub use client::{AwsClient, AwsConfig};
pub use error::{AwsError, Result};
