//! Backblaze B2 Native API client for infrazeug.
//!
//! A typed, async client for the [B2 Native API](https://www.backblaze.com/apidocs/introduction-to-the-b2-native-api).
//! It authenticates with application key ID + secret via `b2_authorize_account`,
//! caches the account auth token, and re-authorizes on expiry.
//!
//! Typed bindings cover:
//!
//! - [`bucket`] — bucket list/create/update/delete
//! - [`application_key`] — application key list/create/delete
//!
//! # Authentication
//!
//! Create an application key in the Backblaze web UI (master key) or via
//! `b2_create_key`, then pass it as [`Credentials::new`].
//!
//! # Example
//!
//! ```no_run
//! use infrazeug_ext_backblaze_api::{BackblazeClient, BackblazeConfig, Credentials};
//! use infrazeug_ext_backblaze_api::bucket::BucketCreate;
//!
//! # async fn run() -> infrazeug_ext_backblaze_api::Result<()> {
//! let client = BackblazeClient::new(BackblazeConfig::new(Credentials::new(
//!     std::env::var("B2_APPLICATION_KEY_ID").unwrap(),
//!     std::env::var("B2_APPLICATION_KEY").unwrap(),
//! )));
//!
//! let account_id = client.account_id().await?;
//! let buckets = client.list_buckets(None).await?;
//! println!("{} buckets in {}", buckets.len(), account_id);
//!
//! let _created = client
//!     .create_bucket(&BucketCreate {
//!         account_id,
//!         bucket_name: "my-logs-bucket".into(),
//!         bucket_type: "allPrivate".into(),
//!         ..Default::default()
//!     })
//!     .await?;
//! # Ok(())
//! # }
//! ```

pub mod application_key;
pub mod auth;
pub mod bucket;
pub mod error;
pub mod types;

mod client;

pub use auth::Credentials;
pub use client::{BackblazeClient, BackblazeConfig, DEFAULT_API_VERSION, DEFAULT_AUTHORIZE_HOST};
pub use error::{BackblazeError, Result};
