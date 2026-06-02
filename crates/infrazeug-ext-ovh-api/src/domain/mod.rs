//! OVHcloud **domain** product bindings.
//!
//! - **v1** ([`v1`]) — classic Domain names API (`/1.0/domain`): registered domains,
//!   DNS zones, contacts.
//! - **v2** — resource-centric API (`/v2/domain`): managed domain names, AllDom, tasks.

pub mod v1;

mod alldom;
mod name;
mod task;

pub use alldom::*;
pub use name::*;
pub use task::*;
pub use v1::{
    ContactAddress, DnsRecordType, DnsZone, DnsZoneRecord, DnsZoneRecordCreate,
    DnsZoneRecordUpdate, DnssecState, DomainContact, DomainOperationTask, DomainService,
    DomainState, NameServer, NameServerInput, NameServersUpdate, ZoneRecordListQuery,
};

use serde::{Deserialize, Serialize};

/// Resource readiness as reported by API v2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceStatus {
    Creating,
    Deleting,
    Error,
    OutOfSync,
    Ready,
    Suspended,
    Unknown,
    Updating,
}
