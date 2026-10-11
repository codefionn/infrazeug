//! Nesting-depth limits for untrusted wire decoding (SOUL: postcard RPC).
//!
//! postcard has no built-in recursion limit, and the derived `Deserialize`
//! impls for recursive types ([`ShellOp::Seq`](crate::op::ShellOp::Seq),
//! [`FileSource::Transform`](crate::source::FileSource::Transform)) recurse on
//! the Rust stack. A crafted frame with deep nesting (~a few bytes per level)
//! overflows the stack, which Rust turns into `abort()` — killing the whole
//! controller or agent process. Every recursive wire position is therefore
//! decoded through a guard that rejects nesting deeper than [`MAX_NESTING`].
//!
//! The guards are `deserialize_with` helpers on the recursive fields; the
//! serialized (wire) form is unchanged.

use serde::{de::Deserialize, Deserializer};

/// Maximum accepted nesting depth for recursive wire types. Generous for real
/// plans (which nest `Seq`/`Transform` single digits deep) and small enough to
/// stay far away from any stack limit at each frame's stack usage.
pub const MAX_NESTING: usize = 64;

thread_local! {
    static NESTING: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Reset token: restores the previous nesting level even if the inner
/// deserialization panics and the panic is caught.
struct DepthGuard(usize);

impl Drop for DepthGuard {
    fn drop(&mut self) {
        NESTING.with(|n| n.set(self.0));
    }
}

fn decode_bounded<'de, D, T, F>(deserializer: D, f: F) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    F: FnOnce(D) -> Result<T, D::Error>,
{
    let prev = NESTING.with(|n| n.get());
    if prev >= MAX_NESTING {
        return Err(serde::de::Error::custom(format!(
            "wire type nested deeper than {MAX_NESTING} levels"
        )));
    }
    NESTING.with(|n| n.set(prev + 1));
    let _guard = DepthGuard(prev);
    f(deserializer)
}

/// Depth-guarded decoder for `Vec<ShellOp>` fields.
pub fn de_shell_ops<'de, D>(deserializer: D) -> Result<Vec<crate::op::ShellOp>, D::Error>
where
    D: Deserializer<'de>,
{
    decode_bounded(deserializer, Vec::<crate::op::ShellOp>::deserialize)
}

/// Depth-guarded decoder for `FileSource` fields.
pub fn de_file_source<'de, D>(deserializer: D) -> Result<crate::source::FileSource, D::Error>
where
    D: Deserializer<'de>,
{
    decode_bounded(deserializer, crate::source::FileSource::deserialize)
}

/// Depth-guarded decoder for `Box<FileSource>` fields.
pub fn de_file_source_boxed<'de, D>(
    deserializer: D,
) -> Result<Box<crate::source::FileSource>, D::Error>
where
    D: Deserializer<'de>,
{
    decode_bounded(deserializer, crate::source::FileSource::deserialize).map(Box::new)
}
