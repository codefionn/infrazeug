//! Read passphrases from flags, files, env, piped stdin, or an interactive prompt.

use infrazeug_core::read_passphrase_prompt;
use infrazeug_migrate::read_passphrase_file;
use std::path::Path;

/// Resolve a passphrase: `inline` → `password_file` → `env_password_file` → stdin / prompt.
pub fn resolve_passphrase(
    inline: Option<String>,
    password_file: Option<&Path>,
    env_password_file: Option<&str>,
    prompt: &str,
) -> anyhow::Result<String> {
    if let Some(p) = inline {
        return Ok(p);
    }
    if let Some(path) = password_file {
        return read_passphrase_file(path).map_err(anyhow::Error::msg);
    }
    if let Some(var) = env_password_file {
        if let Ok(path) = std::env::var(var) {
            return read_passphrase_file(Path::new(&path)).map_err(anyhow::Error::msg);
        }
    }
    read_passphrase_prompt(prompt).map_err(anyhow::Error::msg)
}

/// Resolve an optional secret such as a hardware PIN: use `inline` if given, otherwise
/// prompt once. An empty value (flag or prompt) yields `None` (e.g. rely on built-in UV).
pub fn resolve_optional_secret(
    inline: Option<String>,
    prompt: &str,
) -> anyhow::Result<Option<String>> {
    let value = match inline {
        Some(v) => v,
        None => read_passphrase_prompt(prompt).map_err(anyhow::Error::msg)?,
    };
    Ok(if value.is_empty() { None } else { Some(value) })
}

/// Like [`resolve_passphrase`], but when reading interactively or from stdin requires a matching second entry.
pub fn resolve_new_passphrase(
    inline: Option<String>,
    password_file: Option<&Path>,
    prompt: &str,
    confirm_prompt: &str,
) -> anyhow::Result<String> {
    if let Some(p) = inline {
        return Ok(p);
    }
    if let Some(path) = password_file {
        return read_passphrase_file(path).map_err(anyhow::Error::msg);
    }
    let first = read_passphrase_prompt(prompt).map_err(anyhow::Error::msg)?;
    let second = read_passphrase_prompt(confirm_prompt).map_err(anyhow::Error::msg)?;
    if first != second {
        anyhow::bail!("passphrases do not match");
    }
    Ok(first)
}
