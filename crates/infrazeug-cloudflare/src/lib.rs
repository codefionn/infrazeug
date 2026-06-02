//! Cloudflare tier-1 native nodes for infrazeug playbooks.
//!
//! Thin bridge over [`infrazeug_ext_cloudflare_api`] built on the shared
//! [`infrazeug_resource`] resource interface.
//!
//! # Example
//!
//! ```ignore
//! use infrazeug_api::builder::{self, InfraBuilder};
//! use infrazeug_cloudflare::{client_from_env, CloudflareInfraExt, EnsureDnsRecordInput};
//! use infrazeug_core::id::{MachineId, NodeId};
//! use uuid::Uuid;
//!
//! let local = MachineId(Uuid::new_v4());
//! let dns = NodeId(Uuid::new_v4());
//! let bundle = InfraBuilder::new()
//!     .machine(builder::controller(local))?
//!     .cloudflare(client_from_env()?, local)
//!     .ensure_dns_record(dns, "www-a", EnsureDnsRecordInput {
//!         zone_name: Some("example.com".into()),
//!         name: "www.example.com".into(),
//!         record_type: "A".into(),
//!         content: "192.0.2.1".into(),
//!         proxied: Some(true),
//!         ..Default::default()
//!     })?
//!     .finish();
//! ```

mod builder;
mod client;
mod methods;
mod registry;

pub use builder::{CloudflareInfraBuilder, CloudflareInfraExt};
pub use client::{client_from_env, CloudflareClientSource};
pub use methods::{
    ensure_dns_record, ensure_firewall_access_rule, ensure_kv_namespace, ensure_r2_bucket,
    ensure_waf_custom_rule, ensure_zone_setting, EnsureDnsRecord, EnsureDnsRecordInput,
    EnsureDnsRecordOutput, EnsureFirewallAccessRule, EnsureFirewallAccessRuleInput,
    EnsureFirewallAccessRuleOutput, EnsureKvNamespace, EnsureKvNamespaceInput,
    EnsureKvNamespaceOutput, EnsureR2Bucket, EnsureR2BucketInput, EnsureR2BucketOutput,
    EnsureWafCustomRule, EnsureWafCustomRuleInput, EnsureWafCustomRuleOutput, EnsureZoneSetting,
    EnsureZoneSettingInput, EnsureZoneSettingOutput, ENSURE_DNS_RECORD,
    ENSURE_FIREWALL_ACCESS_RULE, ENSURE_KV_NAMESPACE, ENSURE_R2_BUCKET, ENSURE_WAF_CUSTOM_RULE,
    ENSURE_ZONE_SETTING,
};
pub use registry::method_registry;

pub use infrazeug_ext_cloudflare_api::{Auth, CloudflareClient, CloudflareConfig};
pub use infrazeug_resource::{Drift, Resource, ResourceCtx, ResourceError, ResourceResult};
