//! OVHcloud **Public Cloud** bindings (API v1 `/cloud/project`).
//!
//! Compute (instances), block volumes, and object storage live under a project
//! id returned by [`OvhClient::cloud_projects`]. All modules use the project id
//! as `service_name` in OVH API paths.

mod compute;
mod project;
mod storage;
mod user;
mod volume;

pub use compute::*;
pub use project::*;
pub use storage::*;
pub use user::*;
pub use volume::*;

pub(crate) fn project_path(
    service_name: &str,
    client: &crate::client::OvhClient,
    suffix: &str,
) -> String {
    format!(
        "/cloud/project/{}{suffix}",
        client.encode_segment(service_name)
    )
}

/// Path under a project's **region-scoped** resources
/// (`/cloud/project/{serviceName}/region/{regionName}{suffix}`). Used by the S3
/// object-storage bindings, which live per region rather than per project.
pub(crate) fn region_path(
    service_name: &str,
    region_name: &str,
    client: &crate::client::OvhClient,
    suffix: &str,
) -> String {
    format!(
        "/cloud/project/{}/region/{}{suffix}",
        client.encode_segment(service_name),
        client.encode_segment(region_name)
    )
}
