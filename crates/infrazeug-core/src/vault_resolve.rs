//! Resolve `FileSource::Vault` in `ShellOp` trees at apply time.

use crate::error::{CoreError, Result};
use infrazeug_secrets::{VaultRef, VaultStore};
use infrazeug_shell::{FileSource, ShellOp};
use serde_cbor::Value;
use std::future::Future;
use std::pin::Pin;

/// Whether this op tree contains vault-backed file sources.
pub fn shell_op_contains_vault(op: &ShellOp) -> bool {
    let mut stack = vec![op];
    while let Some(current) = stack.pop() {
        match current {
            ShellOp::WriteFile { content, .. } if file_source_contains_vault(content) => {
                return true;
            }
            ShellOp::VaultWrite { value, .. } if file_source_contains_vault(value) => {
                return true;
            }
            ShellOp::Run { env, .. }
                if env
                    .iter()
                    .any(|entry| file_source_contains_vault(&entry.value)) =>
            {
                return true;
            }
            ShellOp::VaultEnsurePasswordHash { .. } => {}
            ShellOp::Seq { steps } => stack.extend(steps.iter()),
            _ => {}
        }
    }
    false
}

pub async fn resolve_vault_in_shell_op(mut op: ShellOp, store: &mut VaultStore) -> Result<ShellOp> {
    let mut stack: Vec<&mut ShellOp> = vec![&mut op];
    while let Some(current) = stack.pop() {
        match current {
            ShellOp::WriteFile { content, .. } => {
                *content = resolve_file_source(content.clone(), store).await?;
            }
            ShellOp::VaultWrite { value, .. } => {
                *value = resolve_file_source(value.clone(), store).await?;
            }
            ShellOp::Run { env, .. } => {
                for entry in env {
                    entry.value = resolve_file_source(entry.value.clone(), store).await?;
                }
            }
            ShellOp::VaultEnsurePasswordHash { .. } => {}
            ShellOp::Seq { steps } => {
                for step in steps.iter_mut().rev() {
                    stack.push(step);
                }
            }
            _ => {}
        }
    }
    Ok(op)
}

fn file_source_contains_vault(content: &FileSource) -> bool {
    match content {
        FileSource::Vault { .. } | FileSource::VaultYamlSubstitute { .. } => true,
        FileSource::Transform { source, .. } => file_source_contains_vault(source),
        FileSource::Bytes(_)
        | FileSource::RandomBytes { .. }
        | FileSource::RandomPassword(_)
        | FileSource::Capture(_) => false,
    }
}

fn resolve_file_source<'a>(
    content: FileSource,
    store: &'a mut VaultStore,
) -> Pin<Box<dyn Future<Output = Result<FileSource>> + Send + 'a>> {
    Box::pin(async move {
        match content {
            FileSource::Bytes(b) => Ok(FileSource::Bytes(b)),
            FileSource::RandomBytes { .. } | FileSource::RandomPassword(_) => Ok(content),
            FileSource::Capture(_) => Ok(content),
            FileSource::Transform { source, transforms } => {
                let resolved = resolve_file_source(*source, store).await?;
                let FileSource::Bytes(mut bytes) = resolved else {
                    return Ok(FileSource::Transform {
                        source: Box::new(resolved),
                        transforms,
                    });
                };
                infrazeug_shell::resolve::apply_transforms(&mut bytes, &transforms)
                    .map_err(|e| CoreError::other(e.to_string()))?;
                Ok(FileSource::Bytes(bytes))
            }
            FileSource::Vault { file, field } => {
                let reference = match field {
                    Some(f) => VaultRef::field(file, f),
                    None => VaultRef::file(file),
                };
                let raw = store
                    .resolve_field(&reference)
                    .await
                    .map_err(|e| CoreError::other(e.to_string()))?;
                Ok(FileSource::Bytes(vault_value_to_bytes(raw)?))
            }
            FileSource::VaultYamlSubstitute {
                template,
                substitutions,
            } => {
                let mut yaml = template;
                for (key, reference) in &substitutions {
                    let raw = store
                        .resolve_field(reference)
                        .await
                        .map_err(|e| CoreError::other(e.to_string()))?;
                    let secret = vault_value_to_string(raw)?;
                    let quoted = yaml_double_quoted(&secret);
                    let marker = format!("\"\" # vault:{key}");
                    if !yaml.contains(&marker) {
                        return Err(CoreError::other(format!(
                            "helm values vault marker missing: {marker}"
                        )));
                    }
                    yaml = yaml.replace(&marker, &quoted);
                }
                Ok(FileSource::Bytes(yaml.into_bytes()))
            }
        }
    })
}

fn vault_value_to_string(value: Value) -> Result<String> {
    match value {
        Value::Text(s) => Ok(s),
        Value::Bytes(b) => String::from_utf8(b)
            .map_err(|e| CoreError::other(format!("vault value not utf-8: {e}"))),
        other => {
            let s = serde_json::to_string(&other)
                .map_err(|e| CoreError::other(format!("vault value not textual: {e}")))?;
            Ok(s)
        }
    }
}

/// YAML double-quoted scalar for Helm values (passwords may contain special chars).
fn yaml_double_quoted(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub(crate) fn vault_value_to_bytes(value: Value) -> Result<Vec<u8>> {
    match value {
        Value::Text(s) => Ok(s.into_bytes()),
        Value::Bytes(b) => Ok(b),
        other => {
            let s = serde_json::to_string(&other)
                .map_err(|e| CoreError::other(format!("vault value not textual: {e}")))?;
            Ok(s.into_bytes())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_double_quoted_escapes_specials() {
        assert_eq!(yaml_double_quoted("plain"), "\"plain\"");
        assert_eq!(yaml_double_quoted("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
}
