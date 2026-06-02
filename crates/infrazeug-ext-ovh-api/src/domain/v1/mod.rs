//! OVHcloud **Domain names** API v1 (`/1.0/domain`).

mod contact;
mod service;
mod zone;

pub use contact::*;
pub use service::*;
pub use zone::*;

pub(crate) fn domain_path(
    client: &crate::client::OvhClient,
    service_name: &str,
    suffix: &str,
) -> String {
    format!("/domain/{}{suffix}", client.encode_segment(service_name))
}

pub(crate) fn zone_path(
    client: &crate::client::OvhClient,
    zone_name: &str,
    suffix: &str,
) -> String {
    format!("/domain/zone/{}{suffix}", client.encode_segment(zone_name))
}
