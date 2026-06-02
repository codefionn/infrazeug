//! Resolve capture references in `ShellOp` before execution.

use crate::error::{Result, ShellError};
use crate::op::ShellOp;
use crate::source::{
    CaptureRef, FileSource, FileSourceTransform, PasswordHashAlgorithm, PasswordHashSpec,
    RandomPasswordSpec, CAPTURE_MAX_BYTES,
};
use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::seq::SliceRandom;
use rand::{rngs::OsRng, RngCore};
use std::collections::HashMap;
use uuid::Uuid;

/// Lookup captured stdout by `(node, machine)`.
pub trait CaptureLookup: Send + Sync {
    fn capture_stdout(&self, node: Uuid, machine: Uuid) -> Result<Vec<u8>>;
}

impl CaptureLookup for HashMap<(Uuid, Uuid), Vec<u8>> {
    fn capture_stdout(&self, node: Uuid, machine: Uuid) -> Result<Vec<u8>> {
        self.get(&(node, machine)).cloned().ok_or_else(|| {
            ShellError::Other(format!(
                "capture missing for node {node} on machine {machine}"
            ))
        })
    }
}

/// Substitute all `FileSource::Capture` with literal bytes.
pub fn resolve_shell_op(
    op: &ShellOp,
    on_machine: Uuid,
    captures: &dyn CaptureLookup,
) -> Result<ShellOp> {
    Ok(match op {
        ShellOp::WriteFile {
            path,
            content,
            mode,
        } => ShellOp::WriteFile {
            path: path.clone(),
            content: resolve_file_source(content, on_machine, captures)?,
            mode: *mode,
        },
        ShellOp::VaultWrite {
            data_key_id,
            file,
            field,
            value,
            if_absent,
        } => ShellOp::VaultWrite {
            data_key_id: data_key_id.clone(),
            file: file.clone(),
            field: field.clone(),
            value: resolve_file_source(value, on_machine, captures)?,
            if_absent: *if_absent,
        },
        ShellOp::Run { argv, cwd, env } => ShellOp::Run {
            argv: argv.clone(),
            cwd: cwd.clone(),
            env: env
                .iter()
                .map(|entry| {
                    Ok(crate::op::EnvVarSource {
                        name: entry.name.clone(),
                        value: resolve_file_source(&entry.value, on_machine, captures)?,
                    })
                })
                .collect::<Result<_>>()?,
        },
        ShellOp::VaultEnsurePasswordHash {
            data_key_id,
            file,
            password_field,
            hash_field,
            password,
            hash,
        } => ShellOp::VaultEnsurePasswordHash {
            data_key_id: data_key_id.clone(),
            file: file.clone(),
            password_field: password_field.clone(),
            hash_field: hash_field.clone(),
            password: password.clone(),
            hash: hash.clone(),
        },
        ShellOp::Seq { steps } => ShellOp::Seq {
            steps: steps
                .iter()
                .map(|s| resolve_shell_op(s, on_machine, captures))
                .collect::<Result<_>>()?,
        },
        ShellOp::Poll {
            check_argv,
            every,
            timeout,
        } => ShellOp::Poll {
            check_argv: check_argv.clone(),
            every: *every,
            timeout: *timeout,
        },
        other => other.clone(),
    })
}

fn resolve_file_source(
    source: &FileSource,
    on_machine: Uuid,
    captures: &dyn CaptureLookup,
) -> Result<FileSource> {
    match source {
        FileSource::Bytes(b) => Ok(FileSource::Bytes(b.clone())),
        FileSource::RandomBytes { len } => Ok(FileSource::Bytes(random_bytes(*len))),
        FileSource::RandomPassword(spec) => Ok(FileSource::Bytes(random_password(spec)?)),
        FileSource::Capture(r) => {
            let bytes = resolve_capture(r, on_machine, captures)?;
            if bytes.len() > CAPTURE_MAX_BYTES {
                return Err(ShellError::Other(format!(
                    "capture for node {} exceeds {} byte limit ({} bytes)",
                    r.node,
                    CAPTURE_MAX_BYTES,
                    bytes.len()
                )));
            }
            Ok(FileSource::Bytes(bytes))
        }
        FileSource::Transform { source, transforms } => {
            let resolved = resolve_file_source(source, on_machine, captures)?;
            let FileSource::Bytes(mut bytes) = resolved else {
                return Ok(FileSource::Transform {
                    source: Box::new(resolved),
                    transforms: transforms.clone(),
                });
            };
            apply_transforms(&mut bytes, transforms)?;
            Ok(FileSource::Bytes(bytes))
        }
        FileSource::Vault { .. } | FileSource::VaultYamlSubstitute { .. } => Ok(source.clone()),
    }
}

/// Resolve literal generated sources and transforms that do not depend on captures or vaults.
pub fn resolve_literal_file_source(source: &FileSource) -> Result<FileSource> {
    let captures = HashMap::new();
    resolve_file_source(source, Uuid::nil(), &captures)
}

pub fn apply_transforms(bytes: &mut Vec<u8>, transforms: &[FileSourceTransform]) -> Result<()> {
    for transform in transforms {
        match transform {
            FileSourceTransform::Trim => trim_ascii_whitespace(bytes),
            FileSourceTransform::RegexInclude { pattern, capture } => {
                *bytes = regex_include(bytes, pattern, *capture)?;
            }
            FileSourceTransform::RegexExclude { pattern } => {
                regex_exclude(bytes, pattern)?;
            }
            FileSourceTransform::Base64Decode => base64_decode(bytes)?,
            FileSourceTransform::Base64Encode => base64_encode(bytes),
            FileSourceTransform::JsonPointer { path, optional } => {
                json_pointer(bytes, path, *optional)?
            }
            FileSourceTransform::Replace {
                pattern,
                replacement,
            } => regex_replace(bytes, pattern, replacement)?,
            FileSourceTransform::LinePrefix { prefix } => line_prefix_op(bytes, prefix),
            FileSourceTransform::Append { suffix } => bytes.extend_from_slice(suffix),
            FileSourceTransform::Prepend { prefix } => {
                let mut out = prefix.clone();
                out.extend_from_slice(bytes);
                *bytes = out;
            }
            FileSourceTransform::Head { n } => head_op(bytes, *n),
            FileSourceTransform::Tail { n } => tail_op(bytes, *n),
            FileSourceTransform::LineInclude { pattern } => line_include_op(bytes, pattern)?,
            FileSourceTransform::LineExclude { pattern } => line_exclude_op(bytes, pattern)?,
            FileSourceTransform::PasswordHash(spec) => *bytes = hash_password(bytes, spec)?,
        }
    }
    Ok(())
}

fn random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = vec![0; len];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

fn random_password(spec: &RandomPasswordSpec) -> Result<Vec<u8>> {
    let groups = password_groups(spec)?;
    let required = if spec.require_each_selected_class {
        groups.len()
    } else {
        0
    };
    if spec.length < required {
        return Err(ShellError::Other(format!(
            "random password length {} is too short for {required} required character classes",
            spec.length
        )));
    }

    let mut rng = OsRng;
    let mut chars = Vec::with_capacity(spec.length);
    if spec.require_each_selected_class {
        for group in &groups {
            chars.push(*group.choose(&mut rng).expect("non-empty password group"));
        }
    }

    let mut alphabet = Vec::new();
    for group in &groups {
        for ch in group {
            if !alphabet.contains(ch) {
                alphabet.push(*ch);
            }
        }
    }

    while chars.len() < spec.length {
        chars.push(
            *alphabet
                .choose(&mut rng)
                .expect("non-empty password alphabet"),
        );
    }
    chars.shuffle(&mut rng);
    Ok(chars.into_iter().collect::<String>().into_bytes())
}

pub fn hash_password(password: &[u8], spec: &PasswordHashSpec) -> Result<Vec<u8>> {
    let algorithm = match spec.algorithm {
        PasswordHashAlgorithm::Argon2id => Algorithm::Argon2id,
        PasswordHashAlgorithm::Argon2i => Algorithm::Argon2i,
        PasswordHashAlgorithm::Argon2d => Algorithm::Argon2d,
    };
    let params = Params::new(spec.m_cost, spec.t_cost, spec.p_cost, spec.output_len)
        .map_err(|e| ShellError::Other(format!("invalid password hash params: {e}")))?;
    let hasher = Argon2::new(algorithm, Version::V0x13, params);
    let salt = SaltString::generate(&mut OsRng);
    let hash = hasher
        .hash_password(password, &salt)
        .map_err(|e| ShellError::Other(format!("password hash failed: {e}")))?;
    Ok(hash.to_string().into_bytes())
}

fn password_groups(spec: &RandomPasswordSpec) -> Result<Vec<Vec<char>>> {
    let mut groups = Vec::new();
    if spec.lowercase {
        groups.push(('a'..='z').collect());
    }
    if spec.uppercase {
        groups.push(('A'..='Z').collect());
    }
    if spec.numbers {
        groups.push(('0'..='9').collect());
    }
    if !spec.special.is_empty() {
        let mut special = Vec::new();
        for ch in spec.special.chars() {
            if ch.is_control() {
                return Err(ShellError::Other(
                    "random password special characters must not contain control characters".into(),
                ));
            }
            if !special.contains(&ch) {
                special.push(ch);
            }
        }
        groups.push(special);
    }
    if groups.is_empty() {
        return Err(ShellError::Other(
            "random password requires at least one character class".into(),
        ));
    }
    Ok(groups)
}

fn trim_ascii_whitespace(bytes: &mut Vec<u8>) {
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map(|i| i + 1)
        .unwrap_or(start);
    if start > 0 || end < bytes.len() {
        *bytes = bytes[start..end].to_vec();
    }
}

fn regex_include(bytes: &[u8], pattern: &str, capture: Option<usize>) -> Result<Vec<u8>> {
    let re = regex::bytes::Regex::new(pattern)
        .map_err(|e| ShellError::Other(format!("invalid regex {pattern:?}: {e}")))?;
    let captures = re
        .captures(bytes)
        .ok_or_else(|| ShellError::Other(format!("regex include did not match {pattern:?}")))?;
    if let Some(index) = capture {
        return captures
            .get(index)
            .map(|m| m.as_bytes().to_vec())
            .ok_or_else(|| {
                ShellError::Other(format!(
                    "regex include pattern {pattern:?} did not produce capture group {index}"
                ))
            });
    }
    if let Some(group) = captures.get(1) {
        return Ok(group.as_bytes().to_vec());
    }
    captures
        .get(0)
        .map(|m| m.as_bytes().to_vec())
        .ok_or_else(|| ShellError::Other(format!("regex include did not match {pattern:?}")))
}

fn regex_exclude(bytes: &[u8], pattern: &str) -> Result<()> {
    let re = regex::bytes::Regex::new(pattern)
        .map_err(|e| ShellError::Other(format!("invalid regex {pattern:?}: {e}")))?;
    if re.is_match(bytes) {
        return Err(ShellError::Other(format!(
            "regex exclude matched forbidden pattern {pattern:?}"
        )));
    }
    Ok(())
}

fn base64_decode(bytes: &mut Vec<u8>) -> Result<()> {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    *bytes = STANDARD
        .decode(bytes.as_slice())
        .map_err(|e| ShellError::Other(format!("base64 decode failed: {e}")))?;
    Ok(())
}

fn base64_encode(bytes: &mut Vec<u8>) {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    *bytes = STANDARD.encode(bytes.as_slice()).into_bytes();
}

fn json_pointer(bytes: &mut Vec<u8>, pointer: &str, optional: bool) -> Result<()> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| ShellError::Other(format!("json pointer: invalid json: {e}")))?;
    let resolved = match value.pointer(pointer) {
        Some(serde_json::Value::Null) | None if optional => {
            bytes.clear();
            return Ok(());
        }
        Some(v) => v,
        None => {
            return Err(ShellError::Other(format!(
                "json pointer: path {pointer:?} not found"
            )))
        }
    };
    let out = match resolved {
        serde_json::Value::String(s) => s.as_bytes().to_vec(),
        other => serde_json::to_vec(other)
            .map_err(|e| ShellError::Other(format!("json pointer: serialize failed: {e}")))?,
    };
    *bytes = out;
    Ok(())
}

fn regex_replace(bytes: &mut Vec<u8>, pattern: &str, replacement: &str) -> Result<()> {
    let re = regex::bytes::Regex::new(pattern)
        .map_err(|e| ShellError::Other(format!("invalid regex {pattern:?}: {e}")))?;
    *bytes = re
        .replace_all(bytes.as_slice(), replacement.as_bytes())
        .into_owned();
    Ok(())
}

fn line_prefix_op(bytes: &mut Vec<u8>, prefix: &str) {
    let prefix_bytes = prefix.as_bytes();
    let text = String::from_utf8_lossy(bytes);
    let mut out = Vec::with_capacity(bytes.len() + prefix_bytes.len() * 16);
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push(b'\n');
        }
        if !line.is_empty() {
            out.extend_from_slice(prefix_bytes);
            out.extend_from_slice(line.as_bytes());
        }
    }
    *bytes = out;
}

fn head_op(bytes: &mut Vec<u8>, n: usize) {
    let text = String::from_utf8_lossy(bytes);
    let kept: Vec<&str> = text.split('\n').take(n).collect();
    let trailing = text.ends_with('\n') && kept.len() == n;
    let mut out = kept.join("\n");
    if trailing {
        out.push('\n');
    }
    *bytes = out.into_bytes();
}

fn tail_op(bytes: &mut Vec<u8>, n: usize) {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.split('\n').collect();
    let trailing = text.ends_with('\n');
    let skip = lines
        .len()
        .saturating_sub(n)
        .saturating_sub(if trailing { 1 } else { 0 });
    let kept: Vec<&str> = lines.into_iter().skip(skip).take(n).collect();
    let mut out = kept.join("\n");
    if trailing {
        out.push('\n');
    }
    *bytes = out.into_bytes();
}

fn line_include_op(bytes: &mut Vec<u8>, pattern: &str) -> Result<()> {
    let re = regex::bytes::Regex::new(pattern)
        .map_err(|e| ShellError::Other(format!("invalid regex {pattern:?}: {e}")))?;
    let text = String::from_utf8_lossy(bytes);
    let trailing_nl = text.ends_with('\n');
    let lines: Vec<&str> = text.split('\n').filter(|l| !l.is_empty()).collect();
    let kept: Vec<&str> = lines
        .into_iter()
        .filter(|line| re.is_match(line.as_bytes()))
        .collect();
    let mut out = kept.join("\n");
    if trailing_nl {
        out.push('\n');
    }
    *bytes = out.into_bytes();
    Ok(())
}

fn line_exclude_op(bytes: &mut Vec<u8>, pattern: &str) -> Result<()> {
    let re = regex::bytes::Regex::new(pattern)
        .map_err(|e| ShellError::Other(format!("invalid regex {pattern:?}: {e}")))?;
    let text = String::from_utf8_lossy(bytes);
    let trailing_nl = text.ends_with('\n');
    let lines: Vec<&str> = text.split('\n').filter(|l| !l.is_empty()).collect();
    let kept: Vec<&str> = lines
        .into_iter()
        .filter(|line| !re.is_match(line.as_bytes()))
        .collect();
    let mut out = kept.join("\n");
    if trailing_nl {
        out.push('\n');
    }
    *bytes = out.into_bytes();
    Ok(())
}

fn resolve_capture(
    reference: &CaptureRef,
    on_machine: Uuid,
    captures: &dyn CaptureLookup,
) -> Result<Vec<u8>> {
    let source_machine = reference.machine.unwrap_or(on_machine);
    captures.capture_stdout(reference.node, source_machine)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::ShellOp;
    use crate::source::{FileSource, PasswordHashSpec, RandomPasswordSpec};
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn resolves_capture_to_bytes() {
        let node = Uuid::new_v4();
        let machine = Uuid::new_v4();
        let mut map = HashMap::new();
        map.insert((node, machine), b"payload".to_vec());

        let op = ShellOp::write_file(
            "/tmp/out",
            FileSource::capture_on_machine(node, machine),
            0o644,
        );
        let resolved = resolve_shell_op(&op, Uuid::new_v4(), &map).unwrap();
        let ShellOp::WriteFile { content, .. } = resolved else {
            panic!("expected write");
        };
        assert_eq!(content, FileSource::Bytes(b"payload".to_vec()));
    }

    #[test]
    fn resolves_transformed_capture() {
        let node = Uuid::new_v4();
        let machine = Uuid::new_v4();
        let mut map = HashMap::new();
        map.insert((node, machine), b"  api-key\n".to_vec());

        let op = ShellOp::vault_write(
            "prod-runtime",
            "mutable/cloud/images.vault",
            "credentials.access_key",
            FileSource::capture_on_machine(node, machine).trim(),
        );
        let resolved = resolve_shell_op(&op, Uuid::new_v4(), &map).unwrap();
        let ShellOp::VaultWrite { value, .. } = resolved else {
            panic!("expected vault write");
        };
        assert_eq!(value, FileSource::Bytes(b"api-key".to_vec()));
    }

    #[test]
    fn regex_include_extracts_capture_and_exclude_rejects() {
        let mut bytes = b"created api_key=ak_live_123\n".to_vec();
        apply_transforms(
            &mut bytes,
            &[
                FileSourceTransform::RegexInclude {
                    pattern: "api_key=([A-Za-z0-9_]+)".into(),
                    capture: None,
                },
                FileSourceTransform::Trim,
            ],
        )
        .unwrap();
        assert_eq!(bytes, b"ak_live_123");

        let mut bytes = b"created api_key=PLACEHOLDER\n".to_vec();
        let err = apply_transforms(
            &mut bytes,
            &[FileSourceTransform::RegexExclude {
                pattern: "PLACEHOLDER".into(),
            }],
        )
        .unwrap_err();
        assert!(err.to_string().contains("forbidden pattern"));
    }

    #[test]
    fn base64_round_trip() {
        let mut bytes = b"hello world".to_vec();
        apply_transforms(&mut bytes, &[FileSourceTransform::Base64Encode]).unwrap();
        assert_eq!(bytes, b"aGVsbG8gd29ybGQ=");
        apply_transforms(&mut bytes, &[FileSourceTransform::Base64Decode]).unwrap();
        assert_eq!(bytes, b"hello world");
    }

    #[test]
    fn resolves_random_bytes_and_base64() {
        let op = ShellOp::write_file("/tmp/out", FileSource::random_base64(32), 0o600);
        let resolved = resolve_shell_op(&op, Uuid::new_v4(), &HashMap::new()).unwrap();
        let ShellOp::WriteFile {
            content: FileSource::Bytes(bytes),
            ..
        } = resolved
        else {
            panic!("expected resolved bytes");
        };
        assert_eq!(bytes.len(), 44);

        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        let decoded = STANDARD.decode(bytes).unwrap();
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn resolves_random_password_with_required_classes() {
        let spec = RandomPasswordSpec::new(4).special("!@");
        let op = ShellOp::write_random_password("/tmp/out", spec, 0o600);
        let resolved = resolve_shell_op(&op, Uuid::new_v4(), &HashMap::new()).unwrap();
        let ShellOp::WriteFile {
            content: FileSource::Bytes(bytes),
            ..
        } = resolved
        else {
            panic!("expected resolved bytes");
        };
        let password = String::from_utf8(bytes).unwrap();
        assert_eq!(password.chars().count(), 4);
        assert!(password.chars().any(|ch| ch.is_ascii_lowercase()));
        assert!(password.chars().any(|ch| ch.is_ascii_uppercase()));
        assert!(password.chars().any(|ch| ch.is_ascii_digit()));
        assert!(password.chars().any(|ch| "!@".contains(ch)));
    }

    #[test]
    fn random_password_rejects_invalid_specs() {
        let empty = RandomPasswordSpec::new(12)
            .lowercase(false)
            .uppercase(false)
            .numbers(false);
        let err = resolve_literal_file_source(&FileSource::random_password(empty)).unwrap_err();
        assert!(err.to_string().contains("at least one character class"));

        let too_short = RandomPasswordSpec::new(3).special("!");
        let err = resolve_literal_file_source(&FileSource::random_password(too_short)).unwrap_err();
        assert!(err.to_string().contains("too short"));

        let control = RandomPasswordSpec::new(8)
            .lowercase(false)
            .uppercase(false)
            .numbers(false)
            .special("!\n");
        let err = resolve_literal_file_source(&FileSource::random_password(control)).unwrap_err();
        assert!(err.to_string().contains("control characters"));
    }

    #[test]
    fn password_hash_transform_outputs_argon2_phc() {
        let mut bytes = b"secret".to_vec();
        apply_transforms(
            &mut bytes,
            &[FileSourceTransform::PasswordHash(
                PasswordHashSpec::argon2id().m_cost(8).t_cost(1),
            )],
        )
        .unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("$argon2id$v=19$m=8,t=1,p=1$"));
    }

    #[test]
    fn json_pointer_extracts_string_and_number() {
        let mut bytes = br#"{"a":{"b":"val","n":42}}"#.to_vec();
        apply_transforms(
            &mut bytes,
            &[FileSourceTransform::JsonPointer {
                path: "/a/b".into(),
                optional: false,
            }],
        )
        .unwrap();
        assert_eq!(bytes, b"val");

        let mut bytes = br#"{"a":{"b":"val","n":42}}"#.to_vec();
        apply_transforms(
            &mut bytes,
            &[FileSourceTransform::JsonPointer {
                path: "/a/n".into(),
                optional: false,
            }],
        )
        .unwrap();
        assert_eq!(bytes, b"42");
    }

    #[test]
    fn json_pointer_extracts_object() {
        let mut bytes = br#"{"a":{"b":"val"}}"#.to_vec();
        apply_transforms(
            &mut bytes,
            &[FileSourceTransform::JsonPointer {
                path: "/a".into(),
                optional: false,
            }],
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["b"], serde_json::Value::String("val".into()));
    }

    #[test]
    fn json_pointer_missing_errors_unless_optional() {
        // Mandatory pointer on an absent field is a hard error.
        let mut bytes = br#"{"access_key_id":"ak"}"#.to_vec();
        let err = apply_transforms(
            &mut bytes,
            &[FileSourceTransform::JsonPointer {
                path: "/secret_access_key".into(),
                optional: false,
            }],
        )
        .unwrap_err();
        assert!(err.to_string().contains("not found"));

        // Optional pointer yields empty bytes for an absent field …
        let mut bytes = br#"{"access_key_id":"ak"}"#.to_vec();
        apply_transforms(
            &mut bytes,
            &[FileSourceTransform::JsonPointer {
                path: "/secret_access_key".into(),
                optional: true,
            }],
        )
        .unwrap();
        assert!(bytes.is_empty());

        // … and for an explicit JSON null.
        let mut bytes = br#"{"secret_access_key":null}"#.to_vec();
        apply_transforms(
            &mut bytes,
            &[FileSourceTransform::JsonPointer {
                path: "/secret_access_key".into(),
                optional: true,
            }],
        )
        .unwrap();
        assert!(bytes.is_empty());
    }

    #[test]
    fn replace_substitutes_capture_groups() {
        let mut bytes = b"key=old host=here".to_vec();
        apply_transforms(
            &mut bytes,
            &[FileSourceTransform::Replace {
                pattern: r"key=(\w+)".into(),
                replacement: "key=new_$1".into(),
            }],
        )
        .unwrap();
        assert_eq!(bytes, b"key=new_old host=here");
    }

    #[test]
    fn line_prefix_prepends_to_each_line() {
        let mut bytes = b"a\nb\n".to_vec();
        apply_transforms(
            &mut bytes,
            &[FileSourceTransform::LinePrefix {
                prefix: "# ".into(),
            }],
        )
        .unwrap();
        assert_eq!(bytes, b"# a\n# b\n");
    }

    #[test]
    fn append_and_prepend() {
        let mut bytes = b"mid".to_vec();
        apply_transforms(
            &mut bytes,
            &[FileSourceTransform::Append {
                suffix: b" end".to_vec(),
            }],
        )
        .unwrap();
        assert_eq!(bytes, b"mid end");
        apply_transforms(
            &mut bytes,
            &[FileSourceTransform::Prepend {
                prefix: b"start ".to_vec(),
            }],
        )
        .unwrap();
        assert_eq!(bytes, b"start mid end");
    }

    #[test]
    fn head_and_tail_select_lines() {
        let mut bytes = b"a\nb\nc\n".to_vec();
        apply_transforms(&mut bytes, &[FileSourceTransform::Head { n: 2 }]).unwrap();
        assert_eq!(bytes, b"a\nb\n");

        let mut bytes = b"a\nb\nc\n".to_vec();
        apply_transforms(&mut bytes, &[FileSourceTransform::Tail { n: 2 }]).unwrap();
        assert_eq!(bytes, b"b\nc\n");
    }

    #[test]
    fn line_include_keeps_matching() {
        let mut bytes = b"keep this\ndrop this\nkeep too\n".to_vec();
        apply_transforms(
            &mut bytes,
            &[FileSourceTransform::LineInclude {
                pattern: "keep".into(),
            }],
        )
        .unwrap();
        assert_eq!(bytes, b"keep this\nkeep too\n");
    }

    #[test]
    fn line_exclude_drops_matching() {
        let mut bytes = b"keep this\ndrop this\nkeep too\n".to_vec();
        apply_transforms(
            &mut bytes,
            &[FileSourceTransform::LineExclude {
                pattern: "drop".into(),
            }],
        )
        .unwrap();
        assert_eq!(bytes, b"keep this\nkeep too\n");
    }
}
