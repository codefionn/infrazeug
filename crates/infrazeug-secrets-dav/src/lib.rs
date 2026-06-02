//! WebDAV storage backend for the vault (SOUL §6.5).
//!
//! [`WebDavBackend`] implements [`Backend`](infrazeug_secrets::Backend) over HTTP
//! WebDAV (PROPFIND/GET/PUT) for teams that already host secrets on Nextcloud,
//! ownCloud, or similar. Pair with [`MultiBackend`](infrazeug_secrets::MultiBackend)
//! to mirror the same objects to filesystem or S3.

mod backend;

pub use backend::WebDavBackend;
