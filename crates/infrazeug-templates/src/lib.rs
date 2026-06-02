//! infrazeug templating: the compile-time, Rust-native [`template!`] macro plus
//! small render helpers (SOUL §3.3).
//!
//! Templates render to a [`String`] at the call site and are type-checked by
//! rustc — there is no runtime template interpreter, and the rendered bytes
//! flow through the existing `WriteFile` / `FileSource::Bytes` path unchanged.
//!
//! ```
//! use infrazeug_templates::template;
//! let port = 8443u16;
//! let s = template!("listen = {{ port }}\n", port = port);
//! assert_eq!(s, "listen = 8443\n");
//! ```
//!
//! See [`escape`] for config-file quoting helpers usable inside `{{ … }}`.

pub use infrazeug_templates_macros::template;

/// Quoting helpers for embedding untrusted values in rendered config files.
///
/// Each returns a `Display` wrapper, so they compose inside interpolations:
/// `template!("user={{ escape::shell(name) }}\n")`.
pub mod escape {
    use std::fmt;

    /// Single-quote a value for POSIX shell, escaping embedded single quotes.
    pub fn shell(s: &str) -> Shell<'_> {
        Shell(s)
    }

    /// Single-quote a value for YAML scalars (doubling embedded single quotes).
    pub fn yaml_squote(s: &str) -> YamlSquote<'_> {
        YamlSquote(s)
    }

    pub struct Shell<'a>(&'a str);
    impl fmt::Display for Shell<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("'")?;
            for c in self.0.chars() {
                if c == '\'' {
                    f.write_str("'\\''")?;
                } else {
                    f.write_str(c.encode_utf8(&mut [0u8; 4]))?;
                }
            }
            f.write_str("'")
        }
    }

    pub struct YamlSquote<'a>(&'a str);
    impl fmt::Display for YamlSquote<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("'")?;
            for c in self.0.chars() {
                if c == '\'' {
                    f.write_str("''")?;
                } else {
                    f.write_str(c.encode_utf8(&mut [0u8; 4]))?;
                }
            }
            f.write_str("'")
        }
    }
}
