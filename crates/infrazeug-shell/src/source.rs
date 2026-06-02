//! WriteFile content sources and upstream capture references (SOUL §3.3).

use infrazeug_secrets::VaultRef;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Default max capture size (16 MiB, SOUL §3.3.3).
pub const CAPTURE_MAX_BYTES: usize = 16 * 1024 * 1024;

/// Where `WriteFile` bytes come from.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileSource {
    /// Inline bytes known at plan time.
    Bytes(Vec<u8>),
    /// Cryptographically secure random bytes generated at apply/execution time.
    RandomBytes { len: usize },
    /// Cryptographically secure random password generated at apply/execution time.
    RandomPassword(RandomPasswordSpec),
    /// Stdout captured from a completed upstream node.
    Capture(CaptureRef),
    /// Apply byte-level transforms to another source before consumption.
    Transform {
        source: Box<FileSource>,
        transforms: Vec<FileSourceTransform>,
    },
    /// Resolved at apply from an unlocked infrazeug vault store.
    Vault {
        file: String,
        #[serde(default)]
        field: Option<String>,
    },
    /// Helm values YAML with `"" # vault:var_key` placeholders (resolved at apply).
    VaultYamlSubstitute {
        template: String,
        substitutions: BTreeMap<String, VaultRef>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RandomPasswordSpec {
    pub length: usize,
    pub lowercase: bool,
    pub uppercase: bool,
    pub numbers: bool,
    pub special: String,
    /// Ensure at least one character from every selected character class.
    pub require_each_selected_class: bool,
}

impl RandomPasswordSpec {
    pub fn new(length: usize) -> Self {
        Self {
            length,
            lowercase: true,
            uppercase: true,
            numbers: true,
            special: String::new(),
            require_each_selected_class: true,
        }
    }

    pub fn lowercase(mut self, enabled: bool) -> Self {
        self.lowercase = enabled;
        self
    }

    pub fn uppercase(mut self, enabled: bool) -> Self {
        self.uppercase = enabled;
        self
    }

    pub fn numbers(mut self, enabled: bool) -> Self {
        self.numbers = enabled;
        self
    }

    pub fn special(mut self, chars: impl Into<String>) -> Self {
        self.special = chars.into();
        self
    }

    pub fn require_each_selected_class(mut self, enabled: bool) -> Self {
        self.require_each_selected_class = enabled;
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PasswordHashAlgorithm {
    Argon2id,
    Argon2i,
    Argon2d,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PasswordHashSpec {
    pub algorithm: PasswordHashAlgorithm,
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
    #[serde(default)]
    pub output_len: Option<usize>,
}

impl PasswordHashSpec {
    pub fn argon2id() -> Self {
        Self {
            algorithm: PasswordHashAlgorithm::Argon2id,
            m_cost: 19_456,
            t_cost: 2,
            p_cost: 1,
            output_len: None,
        }
    }

    pub fn argon2i() -> Self {
        Self {
            algorithm: PasswordHashAlgorithm::Argon2i,
            ..Self::argon2id()
        }
    }

    pub fn argon2d() -> Self {
        Self {
            algorithm: PasswordHashAlgorithm::Argon2d,
            ..Self::argon2id()
        }
    }

    pub fn m_cost(mut self, m_cost: u32) -> Self {
        self.m_cost = m_cost;
        self
    }

    pub fn t_cost(mut self, t_cost: u32) -> Self {
        self.t_cost = t_cost;
        self
    }

    pub fn p_cost(mut self, p_cost: u32) -> Self {
        self.p_cost = p_cost;
        self
    }

    pub fn output_len(mut self, output_len: usize) -> Self {
        self.output_len = Some(output_len);
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileSourceTransform {
    Trim,
    /// Replace the source with the first regex match. If `capture` is set,
    /// that capture group is used; otherwise capture group 1 is preferred when
    /// present, then the full match.
    RegexInclude {
        pattern: String,
        #[serde(default)]
        capture: Option<usize>,
    },
    /// Reject the source if the regex matches; otherwise leave it unchanged.
    RegexExclude {
        pattern: String,
    },
    Base64Decode,
    Base64Encode,
    JsonPointer {
        path: String,
        /// When the pointer resolves to nothing (path absent or JSON `null`),
        /// yield empty bytes instead of erroring. A downstream [`crate::op::ShellOp::VaultWrite`]
        /// treats an empty value as "nothing to store" and skips the write.
        #[serde(default)]
        optional: bool,
    },
    Replace {
        pattern: String,
        replacement: String,
    },
    LinePrefix {
        prefix: String,
    },
    Append {
        suffix: Vec<u8>,
    },
    Prepend {
        prefix: Vec<u8>,
    },
    Head {
        n: usize,
    },
    Tail {
        n: usize,
    },
    LineInclude {
        pattern: String,
    },
    LineExclude {
        pattern: String,
    },
    PasswordHash(PasswordHashSpec),
}

impl FileSource {
    pub fn bytes(data: impl Into<Vec<u8>>) -> Self {
        Self::Bytes(data.into())
    }

    pub fn random_bytes(len: usize) -> Self {
        Self::RandomBytes { len }
    }

    pub fn random_base64(len: usize) -> Self {
        Self::random_bytes(len).base64_encode()
    }

    pub fn random_password(spec: RandomPasswordSpec) -> Self {
        Self::RandomPassword(spec)
    }

    /// Capture stdout from `node` on the same machine as this op runs.
    pub fn capture_same_machine(node: Uuid) -> Self {
        Self::Capture(CaptureRef {
            node,
            machine: None,
        })
    }

    /// Capture stdout from `node` on a specific machine (cross-machine download).
    pub fn capture_on_machine(node: Uuid, machine: Uuid) -> Self {
        Self::Capture(CaptureRef {
            node,
            machine: Some(machine),
        })
    }

    /// Vault file path under `vault-store/files/` and optional dotted field path.
    pub fn vault(file: impl Into<String>, field: impl Into<String>) -> Self {
        Self::Vault {
            file: file.into(),
            field: Some(field.into()),
        }
    }

    pub fn trim(self) -> Self {
        self.transform(FileSourceTransform::Trim)
    }

    pub fn regex_include(self, pattern: impl Into<String>) -> Self {
        self.transform(FileSourceTransform::RegexInclude {
            pattern: pattern.into(),
            capture: None,
        })
    }

    pub fn regex_include_capture(self, pattern: impl Into<String>, capture: usize) -> Self {
        self.transform(FileSourceTransform::RegexInclude {
            pattern: pattern.into(),
            capture: Some(capture),
        })
    }

    pub fn regex_exclude(self, pattern: impl Into<String>) -> Self {
        self.transform(FileSourceTransform::RegexExclude {
            pattern: pattern.into(),
        })
    }

    pub fn base64_decode(self) -> Self {
        self.transform(FileSourceTransform::Base64Decode)
    }

    pub fn base64_encode(self) -> Self {
        self.transform(FileSourceTransform::Base64Encode)
    }

    pub fn json_pointer(self, path: impl Into<String>) -> Self {
        self.transform(FileSourceTransform::JsonPointer {
            path: path.into(),
            optional: false,
        })
    }

    /// Like [`json_pointer`](Self::json_pointer) but yields empty bytes (rather
    /// than erroring) when the path is absent or `null`. Use for capture fields
    /// that are only sometimes present — e.g. a secret returned once at creation
    /// — so a re-run that no longer carries the field skips the write instead of
    /// failing.
    pub fn json_pointer_optional(self, path: impl Into<String>) -> Self {
        self.transform(FileSourceTransform::JsonPointer {
            path: path.into(),
            optional: true,
        })
    }

    pub fn replace(self, pattern: impl Into<String>, replacement: impl Into<String>) -> Self {
        self.transform(FileSourceTransform::Replace {
            pattern: pattern.into(),
            replacement: replacement.into(),
        })
    }

    pub fn line_prefix(self, prefix: impl Into<String>) -> Self {
        self.transform(FileSourceTransform::LinePrefix {
            prefix: prefix.into(),
        })
    }

    pub fn append(self, suffix: impl Into<Vec<u8>>) -> Self {
        self.transform(FileSourceTransform::Append {
            suffix: suffix.into(),
        })
    }

    pub fn prepend(self, prefix: impl Into<Vec<u8>>) -> Self {
        self.transform(FileSourceTransform::Prepend {
            prefix: prefix.into(),
        })
    }

    pub fn head(self, n: usize) -> Self {
        self.transform(FileSourceTransform::Head { n })
    }

    pub fn tail(self, n: usize) -> Self {
        self.transform(FileSourceTransform::Tail { n })
    }

    pub fn line_include(self, pattern: impl Into<String>) -> Self {
        self.transform(FileSourceTransform::LineInclude {
            pattern: pattern.into(),
        })
    }

    pub fn line_exclude(self, pattern: impl Into<String>) -> Self {
        self.transform(FileSourceTransform::LineExclude {
            pattern: pattern.into(),
        })
    }

    pub fn password_hash(self, spec: PasswordHashSpec) -> Self {
        self.transform(FileSourceTransform::PasswordHash(spec))
    }

    pub fn transform(self, transform: FileSourceTransform) -> Self {
        match self {
            Self::Transform {
                source,
                mut transforms,
            } => {
                transforms.push(transform);
                Self::Transform { source, transforms }
            }
            source => Self::Transform {
                source: Box::new(source),
                transforms: vec![transform],
            },
        }
    }
}

/// Reference to an upstream node's captured stdout.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureRef {
    pub node: Uuid,
    /// When `None`, use the machine executing the consuming op.
    #[serde(default)]
    pub machine: Option<Uuid>,
}

/// Collect every capture reference in a `ShellOp` tree (for plan-time lint).
pub fn capture_refs(op: &crate::op::ShellOp) -> Vec<CaptureRef> {
    let mut out = Vec::new();
    collect_capture_refs(op, &mut out);
    out
}

fn collect_capture_refs(op: &crate::op::ShellOp, out: &mut Vec<CaptureRef>) {
    match op {
        crate::op::ShellOp::WriteFile {
            content: FileSource::Capture(r),
            ..
        } => {
            out.push(r.clone());
        }
        crate::op::ShellOp::WriteFile { content, .. } => {
            collect_capture_refs_source(content, out);
        }
        crate::op::ShellOp::VaultWrite { value, .. } => {
            collect_capture_refs_source(value, out);
        }
        crate::op::ShellOp::Run { env, .. } => {
            for entry in env {
                collect_capture_refs_source(&entry.value, out);
            }
        }
        crate::op::ShellOp::VaultEnsurePasswordHash { .. } => {}
        crate::op::ShellOp::Seq { steps } => {
            for s in steps {
                collect_capture_refs(s, out);
            }
        }
        _ => {}
    }
}

fn collect_capture_refs_source(source: &FileSource, out: &mut Vec<CaptureRef>) {
    match source {
        FileSource::Capture(r) => out.push(r.clone()),
        FileSource::Transform { source, .. } => collect_capture_refs_source(source, out),
        FileSource::Bytes(_)
        | FileSource::RandomBytes { .. }
        | FileSource::RandomPassword(_)
        | FileSource::Vault { .. }
        | FileSource::VaultYamlSubstitute { .. } => {}
    }
}
