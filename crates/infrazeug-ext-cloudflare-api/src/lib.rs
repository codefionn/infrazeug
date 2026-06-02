//! Cloudflare API client for infrazeug.
//!
//! A typed, async client for the [Cloudflare API v4](https://developers.cloudflare.com/api/).
//! It supports API-token (`Authorization: Bearer …`) or legacy global-key
//! (`X-Auth-Email` + `X-Auth-Key`) authentication and exposes typed bindings for
//! the surfaces infrazeug uses today:
//!
//! - [`user`] — token verification and account profile
//! - [`zones`] — zone listing and lookup
//! - [`dns_record`] — DNS record CRUD inside a zone
//! - [`zone_setting`] — per-zone settings (SSL, always_use_https, …)
//! - [`firewall_access_rule`] — zone IP access rules
//! - [`ruleset`] — Rulesets API (WAF custom rules, redirects)
//! - [`account`] — account listing and id resolution
//! - [`r2_bucket`] — R2 object-storage buckets
//! - [`kv_namespace`] — Workers KV namespaces
//!
//! # Authentication
//!
//! Create a scoped API token in the Cloudflare dashboard (recommended) and pass it
//! as [`Auth::token`]. For legacy automation, use [`Auth::global_key`].
//!
//! # Example
//!
//! ```no_run
//! use infrazeug_ext_cloudflare_api::{Auth, CloudflareClient, CloudflareConfig, ListQuery};
//! use infrazeug_ext_cloudflare_api::dns_record::DnsRecord;
//!
//! # async fn run() -> infrazeug_ext_cloudflare_api::Result<()> {
//! let client = CloudflareClient::new(CloudflareConfig::new(Auth::token(
//!     std::env::var("CLOUDFLARE_API_TOKEN").unwrap(),
//! )));
//!
//! let zone_id = client.zone_id_by_name("example.com").await?;
//! let records = client.dns_records(&zone_id, &ListQuery::default()).await?;
//! println!("{} records in {}", records.len(), zone_id);
//!
//! let _created = client
//!     .create_dns_record(
//!         &zone_id,
//!         &DnsRecord {
//!             name: "www.example.com".into(),
//!             record_type: "A".into(),
//!             content: "192.0.2.1".into(),
//!             proxied: Some(true),
//!             ..Default::default()
//!         },
//!     )
//!     .await?;
//! # Ok(())
//! # }
//! ```

pub mod account;
pub mod dns_record;
pub mod error;
pub mod firewall_access_rule;
pub mod kv_namespace;
pub mod r2_bucket;
pub mod ruleset;
pub mod types;
pub mod user;
pub mod zone_setting;
pub mod zones;

mod auth;
mod client;

pub use auth::Auth;
pub use client::{CloudflareClient, CloudflareConfig, DEFAULT_HOST};
pub use error::{CloudflareError, Result};
pub use types::{ApiErrorEntry, CloudflareResponse, ListQuery, ResultInfo};
