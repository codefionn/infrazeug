//! Plaintext-secret lint pass (SOUL: secrets live in the vault, never the repo).
//!
//! Walks the built [`Infra`](crate::infra::Infra) — var sets, shell ops, native
//! method inputs — and reports values that look like secrets embedded in
//! plaintext. Since playbooks are code-first, anything hard-coded in the repo
//! surfaces here. The fix is always the same: store the value in the vault and
//! reference it (`VarValue::Vault`, `FileSource::Vault`, or a `SecretSource`).
//!
//! Three detector classes:
//! - **Known token shapes** (PEM private keys, JWTs, basic-auth and webhook
//!   URLs, and provider token prefixes — AWS, Azure, Google, DigitalOcean,
//!   GitHub, GitLab, Slack, Stripe, HashiCorp Vault, Tailscale, npm/PyPI,
//!   and more; see [`PREFIX_RULES`]) are errors wherever they appear.
//! - **Secret-named keys** (`db_password`, `api_key`, …) holding an inline
//!   string are errors in var sets, native inputs, and argv assignments, and
//!   warnings inside generated file content (config text is more ambiguous).
//! - **High-entropy strings** that look like random token material under a
//!   non-secret key name are advisory warnings (see [`looks_random`]).

use crate::error::CoreError;
use crate::infra::Infra;
use crate::lint::LintReport;
use crate::node::{Node, NodeBody};
use crate::varset::{VarSet, VarValue};
use infrazeug_shell::{FileSource, ShellOp};

const HELP: &str = "store the value in the vault and reference it \
    (VarValue::Vault / FileSource::Vault / SecretSource) instead of plaintext";

/// Scan every var set, shell op, and native input in `infra` for plaintext
/// secrets, recording findings on `report`.
pub fn collect_plaintext_secrets(infra: &Infra, report: &mut LintReport) {
    scan_varset(&infra.global_vars, "global vars", report);
    for group in &infra.groups {
        scan_varset(&group.vars, &format!("group `{}` vars", group.name), report);
    }
    for machine in &infra.machines {
        scan_varset(
            &machine.vars,
            &format!("machine `{}` vars", machine.name),
            report,
        );
    }
    for node in &infra.nodes {
        scan_node(node, &format!("node `{}`", node.name), report);
    }
    for group in &infra.dynamic_groups {
        for node in &group.template {
            scan_node(
                node,
                &format!(
                    "dynamic group `{}` template node `{}`",
                    group.label, node.name
                ),
                report,
            );
        }
    }
}

fn scan_node(node: &Node, location: &str, report: &mut LintReport) {
    match &node.body {
        NodeBody::Shell(op) => scan_shell_op(op, location, report),
        NodeBody::Native { method_id, input } => scan_cbor(
            input,
            None,
            &format!("{location} native input for `{method_id}`"),
            report,
        ),
        NodeBody::Barrier | NodeBody::Begin | NodeBody::Finish | NodeBody::Connect => {}
    }
}

fn scan_varset(vars: &VarSet, location: &str, report: &mut LintReport) {
    for (key, entry) in &vars.entries {
        scan_var_value(&entry.value, &key.0, location, report);
    }
}

fn scan_var_value(value: &VarValue, key: &str, location: &str, report: &mut LintReport) {
    match value {
        VarValue::Scalar(v) => {
            if let Some(s) = v.as_str() {
                scan_keyed_string(key, s, &format!("{location} entry `{key}`"), report);
            }
        }
        // Vault-backed entries are exactly the sanctioned form.
        VarValue::Vault(_) => {}
        VarValue::List(items) => {
            for item in items {
                scan_var_value(item, key, location, report);
            }
        }
        VarValue::Map(map) => {
            for (k, v) in map {
                scan_var_value(v, k, &format!("{location} entry `{key}`"), report);
            }
        }
    }
}

fn scan_shell_op(op: &ShellOp, location: &str, report: &mut LintReport) {
    match op {
        ShellOp::Run { argv, env, .. } => {
            scan_argv(argv, location, report);
            for entry in env {
                let env_location = format!("{location} env `{}`", entry.name);
                match &entry.value {
                    // An inline env value sits under a name: apply the keyed
                    // rules, so `DB_PASSWORD` with literal bytes is an error.
                    FileSource::Bytes(bytes) => {
                        if let Ok(text) = std::str::from_utf8(bytes) {
                            scan_keyed_string(&entry.name, text, &env_location, report);
                        }
                    }
                    other => scan_file_source(other, &env_location, report),
                }
            }
        }
        ShellOp::Poll { check_argv, .. } => scan_argv(check_argv, location, report),
        ShellOp::Seq { steps } => {
            for step in steps {
                scan_shell_op(step, location, report);
            }
        }
        ShellOp::WriteFile { path, content, .. } => scan_file_source(
            content,
            &format!("{location} content for `{}`", path.display()),
            report,
        ),
        ShellOp::VaultWrite { value, file, .. } => scan_file_source(
            value,
            &format!("{location} vault write to `{file}`"),
            report,
        ),
        ShellOp::ReadFile { .. }
        | ShellOp::VaultEnsurePasswordHash { .. }
        | ShellOp::EnsureDir { .. }
        | ShellOp::SyncDir { .. } => {}
    }
}

fn scan_argv(argv: &[String], location: &str, report: &mut LintReport) {
    for arg in argv {
        if let Some(kind) = classify_token(arg) {
            report.error(
                CoreError::PlaintextSecret {
                    location: format!("{location} argv"),
                    what: kind.to_string(),
                },
                HELP.to_string(),
            );
            continue;
        }
        // `--db-password=hunter2` / `DB_PASSWORD=hunter2` style assignments.
        if let Some((key, value)) = arg.split_once('=') {
            let key = key.trim_start_matches('-');
            if secret_like_key(key) && plausible_secret_value(value) {
                report.error(
                    CoreError::PlaintextSecret {
                        location: format!("{location} argv"),
                        what: format!("inline value for secret-like key `{key}`"),
                    },
                    HELP.to_string(),
                );
                continue;
            }
        }
        // One advisory per arg for unrecognized random-looking material
        // (an arg may be a whole inline script, so check word-by-word).
        let arg = without_public_pem_blocks(arg);
        if let Some(word) = arg
            .split_whitespace()
            .map(|w| {
                w.trim_start_matches('-')
                    .split_once('=')
                    .map_or(w, |(_, v)| v)
            })
            .find(|w| looks_random(w))
        {
            report.warning(
                CoreError::PlaintextSecret {
                    location: format!("{location} argv"),
                    what: format!(
                        "high-entropy token `{}…` (possible secret material)",
                        &word[..8]
                    ),
                },
                HELP.to_string(),
            );
        }
    }
}

fn scan_file_source(source: &FileSource, location: &str, report: &mut LintReport) {
    match source {
        FileSource::Bytes(bytes) => {
            if let Ok(text) = std::str::from_utf8(bytes) {
                scan_file_text(text, location, report);
            }
        }
        FileSource::Transform { source, .. } => scan_file_source(source, location, report),
        FileSource::VaultYamlSubstitute { template, .. } => {
            scan_file_text(template, location, report)
        }
        FileSource::RandomBytes { .. }
        | FileSource::RandomPassword(_)
        | FileSource::Capture(_)
        | FileSource::Vault { .. } => {}
    }
}

/// Inline file content: known token shapes are errors; `key = value` lines with
/// a secret-like key are warnings (config text trips heuristics more easily).
fn scan_file_text(text: &str, location: &str, report: &mut LintReport) {
    if let Some(kind) = classify_token(text) {
        report.error(
            CoreError::PlaintextSecret {
                location: location.to_string(),
                what: kind.to_string(),
            },
            HELP.to_string(),
        );
        return;
    }
    for line in text.lines() {
        let Some((key, value)) = split_assignment(line) else {
            continue;
        };
        if secret_like_key(key) && plausible_secret_value(value) {
            report.warning(
                CoreError::PlaintextSecret {
                    location: location.to_string(),
                    what: format!("inline value for secret-like key `{key}`"),
                },
                HELP.to_string(),
            );
        } else if looks_random(value) {
            report.warning(
                CoreError::PlaintextSecret {
                    location: location.to_string(),
                    what: format!("high-entropy value for key `{key}` (possible secret material)"),
                },
                HELP.to_string(),
            );
        }
    }
}

fn split_assignment(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(['=', ':'])?;
    let key = key.trim().trim_start_matches('-').trim_matches('"');
    let mut value = value.trim();
    // Drop a trailing comment so the vault placeholder convention
    // (`apiKey: "" # vault:var_key`) is judged by its empty value, not the
    // comment text.
    if let Some((before, _)) = value.split_once(" #") {
        value = before.trim();
    }
    if value.starts_with('#') {
        value = "";
    }
    let value = value.trim_matches('"').trim_matches('\'');
    if key.is_empty() || key.contains(char::is_whitespace) {
        return None;
    }
    Some((key, value))
}

fn scan_cbor(
    value: &serde_cbor::Value,
    key: Option<&str>,
    location: &str,
    report: &mut LintReport,
) {
    use serde_cbor::Value;
    match value {
        Value::Text(s) => {
            scan_keyed_string(key.unwrap_or(""), s, &keyed_location(location, key), report)
        }
        Value::Array(items) => {
            for item in items {
                scan_cbor(item, key, location, report);
            }
        }
        Value::Map(map) => {
            for (k, v) in map {
                let field = match k {
                    Value::Text(t) => Some(t.as_str()),
                    _ => None,
                };
                scan_cbor(v, field, location, report);
            }
        }
        Value::Tag(_, inner) => scan_cbor(inner, key, location, report),
        _ => {}
    }
}

fn keyed_location(location: &str, key: Option<&str>) -> String {
    match key {
        Some(k) if !k.is_empty() => format!("{location} field `{k}`"),
        _ => location.to_string(),
    }
}

/// A string sitting under a named key: known token shapes and secret-like key
/// names are both blocking errors here.
fn scan_keyed_string(key: &str, value: &str, location: &str, report: &mut LintReport) {
    if let Some(kind) = classify_token(value) {
        report.error(
            CoreError::PlaintextSecret {
                location: location.to_string(),
                what: kind.to_string(),
            },
            HELP.to_string(),
        );
        return;
    }
    if secret_like_key(key) && plausible_secret_value(value) {
        report.error(
            CoreError::PlaintextSecret {
                location: location.to_string(),
                what: format!("inline value for secret-like key `{key}`"),
            },
            HELP.to_string(),
        );
    } else if looks_random(value) {
        report.warning(
            CoreError::PlaintextSecret {
                location: location.to_string(),
                what: format!("high-entropy value for key `{key}` (possible secret material)"),
            },
            HELP.to_string(),
        );
    }
}

/// Heuristic for unrecognized but random-looking token material: a single
/// base64ish word of 32+ chars whose Shannon entropy clears the bar that
/// orderly data stays under — hex digests cap at 4.0 bits/char, UUIDs and
/// English-ish identifiers sit lower still, while random base64/alphanumeric
/// secrets land near 5–6. Advisory only (reported as a warning).
fn looks_random(value: &str) -> bool {
    const MIN_LEN: usize = 32;
    const MIN_BITS_PER_CHAR: f64 = 4.5;
    if value.len() < MIN_LEN || !Charset::StdBase64.matches(value) {
        return false;
    }
    // SSH wire-format blobs (public keys, known_hosts entries) start with the
    // base64 of a length prefix; they are high-entropy but not secret.
    if value.starts_with("AAAA") {
        return false;
    }
    // Random token material virtually always mixes letters and digits.
    if !value.chars().any(|c| c.is_ascii_digit()) || !value.chars().any(|c| c.is_ascii_alphabetic())
    {
        return false;
    }
    shannon_entropy(value) >= MIN_BITS_PER_CHAR
}

/// Strip PEM blocks holding *public* objects (certificates, public keys,
/// CSRs) so their base64 bodies don't trip the entropy heuristic. Blocks whose
/// label names a private key are kept — [`classify_token`] reports those as
/// errors before the entropy pass runs.
fn without_public_pem_blocks(text: &str) -> String {
    const BEGIN: &str = "-----BEGIN ";
    const DASHES: &str = "-----";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(BEGIN) {
        let after_begin = &rest[start + BEGIN.len()..];
        let Some(label_end) = after_begin.find(DASHES) else {
            break;
        };
        let label = &after_begin[..label_end];
        let end_marker = format!("-----END {label}-----");
        let Some(end) = rest[start..].find(&end_marker) else {
            break;
        };
        let block_end = start + end + end_marker.len();
        if label.contains("PRIVATE KEY") {
            out.push_str(&rest[..block_end]);
        } else {
            out.push_str(&rest[..start]);
            out.push(' ');
        }
        rest = &rest[block_end..];
    }
    out.push_str(rest);
    out
}

/// Shannon entropy in bits per character.
fn shannon_entropy(s: &str) -> f64 {
    let mut counts = std::collections::BTreeMap::new();
    let mut len = 0usize;
    for c in s.chars() {
        *counts.entry(c).or_insert(0usize) += 1;
        len += 1;
    }
    let len = len as f64;
    counts
        .values()
        .map(|&n| {
            let p = n as f64 / len;
            -p * p.log2()
        })
        .sum()
}

/// Does `key` name a secret? Decided on the *final* word of the identifier so
/// `password_file`, `token_url`, or `PasswordAuthentication` do not trip it,
/// while `db_password`, `api_key`, and `clientSecret` do.
fn secret_like_key(key: &str) -> bool {
    let words = split_ident(key);
    let Some(last) = words.last() else {
        return false;
    };
    match last.as_str() {
        "password" | "passwd" | "pass" | "secret" | "token" | "credential" | "credentials"
        | "apikey" => true,
        // Bare `key` only counts when the preceding word marks it as a
        // credential: covers `api_key`, `secret_access_key`, and the provider
        // spellings used in this workspace (`application_key` for OVH/B2,
        // `consumer_key` for OVH, `account_key` for Azure storage).
        "key" => {
            let prev = words.len().checked_sub(2).map(|i| words[i].as_str());
            matches!(
                prev,
                Some(
                    "api"
                        | "access"
                        | "secret"
                        | "private"
                        | "auth"
                        | "app"
                        | "application"
                        | "consumer"
                        | "account"
                        | "license"
                )
            )
        }
        _ => false,
    }
}

/// Split `db_password`, `db-password`, `clientSecret`, `PasswordAuthentication`
/// into lowercase words.
fn split_ident(ident: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    // Track the previous char's original case: a camelCase boundary is an
    // uppercase char following a lowercase one, so `DB_PASSWORD` stays whole
    // while `clientSecret` splits.
    let mut prev_lower = false;
    for c in ident.chars() {
        if c == '_' || c == '-' || c == '.' {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            prev_lower = false;
        } else {
            if c.is_uppercase() && prev_lower && !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            current.push(c.to_ascii_lowercase());
            prev_lower = c.is_lowercase();
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Filter out values that are clearly not embedded secrets: empty, template
/// placeholders, env/variable references, file paths, and config keywords.
fn plausible_secret_value(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    if value.starts_with('$') || value.contains("{{") || value.contains("${") {
        return false;
    }
    // Apply-time substitution tokens (e.g. `LIVEKIT_API_SECRET_PLACEHOLDER`)
    // are stand-ins for vault material, not secrets.
    if value.contains("PLACEHOLDER") {
        return false;
    }
    if value.starts_with('/') || value.starts_with("./") || value.starts_with("~/") {
        return false;
    }
    !matches!(
        value.to_ascii_lowercase().as_str(),
        "yes"
            | "no"
            | "true"
            | "false"
            | "on"
            | "off"
            | "none"
            | "null"
            | "disabled"
            | "enabled"
            | "prompt"
            | "ask"
            | "prohibit-password"
            | "without-password"
    )
}

/// Match well-known secret token shapes. Returns a short description of what
/// was recognized, or `None`.
fn classify_token(s: &str) -> Option<&'static str> {
    if s.contains("-----BEGIN ") && s.contains("PRIVATE KEY-----") {
        return Some("PEM private key");
    }
    for word in s.split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',') {
        if let Some(kind) = classify_word(word) {
            return Some(kind);
        }
    }
    if let Some(rest) = s.split_once("://").map(|(_, r)| r) {
        let authority = rest.split('/').next().unwrap_or("");
        if let Some((userinfo, _)) = authority.rsplit_once('@') {
            if let Some((_, pass)) = userinfo.split_once(':') {
                if !pass.is_empty() && !pass.starts_with('$') && !pass.contains("{{") {
                    return Some("URL with embedded basic-auth password");
                }
            }
        }
    }
    None
}

/// Allowed alphabet for the characters following a token prefix.
#[derive(Clone, Copy)]
enum Charset {
    /// `[A-Za-z0-9_=-]` — base64url-ish, the common token alphabet.
    Base64Url,
    /// `[0-9a-fA-F]` — hex tails; the strictest, used where the prefix alone
    /// is too generic to be trusted (e.g. Mailgun `key-`, Twilio `SK`).
    Hex,
    /// `Base64Url` plus `.` — for dotted tokens like SendGrid / Google OAuth.
    Dotted,
    /// `Base64Url` plus `+` and `/` — classic base64 alphabet, used by the
    /// entropy heuristic where the encoding flavor is unknown.
    StdBase64,
}

impl Charset {
    fn matches(self, tail: &str) -> bool {
        tail.chars().all(|c| match self {
            Charset::Base64Url => c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '='),
            Charset::Hex => c.is_ascii_hexdigit(),
            Charset::Dotted => c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '=' | '.'),
            Charset::StdBase64 => {
                c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '=' | '+' | '/')
            }
        })
    }
}

/// Prefix-shaped tokens: (prefix, minimum tail length, tail alphabet, kind).
/// Tail minimums are chosen so ordinary identifiers sharing a prefix
/// (`key-value`, `secret_access_key`) stay well below the bar.
const PREFIX_RULES: &[(&str, usize, Charset, &str)] = &[
    ("ghp_", 20, Charset::Base64Url, "GitHub token"),
    ("gho_", 20, Charset::Base64Url, "GitHub token"),
    ("ghu_", 20, Charset::Base64Url, "GitHub token"),
    ("ghs_", 20, Charset::Base64Url, "GitHub token"),
    ("ghr_", 20, Charset::Base64Url, "GitHub token"),
    ("github_pat_", 20, Charset::Base64Url, "GitHub token"),
    ("glpat-", 20, Charset::Base64Url, "GitLab token"),
    ("gldt-", 20, Charset::Base64Url, "GitLab deploy token"),
    ("glrt-", 20, Charset::Base64Url, "GitLab runner token"),
    ("xoxb-", 15, Charset::Base64Url, "Slack token"),
    ("xoxp-", 15, Charset::Base64Url, "Slack token"),
    ("xoxa-", 15, Charset::Base64Url, "Slack token"),
    ("xoxr-", 15, Charset::Base64Url, "Slack token"),
    ("xoxs-", 15, Charset::Base64Url, "Slack token"),
    ("sk_live_", 16, Charset::Base64Url, "Stripe live key"),
    ("rk_live_", 16, Charset::Base64Url, "Stripe live key"),
    ("sk_test_", 16, Charset::Base64Url, "Stripe test key"),
    ("rk_test_", 16, Charset::Base64Url, "Stripe test key"),
    ("whsec_", 24, Charset::Base64Url, "Stripe webhook secret"),
    ("sq0atp-", 20, Charset::Base64Url, "Square access token"),
    ("sq0csp-", 20, Charset::Base64Url, "Square client secret"),
    ("sk-proj-", 20, Charset::Base64Url, "AI provider API key"),
    ("sk-ant-", 20, Charset::Base64Url, "AI provider API key"),
    ("sk-", 40, Charset::Base64Url, "AI provider API key"),
    (
        "GOCSPX-",
        20,
        Charset::Base64Url,
        "Google OAuth client secret",
    ),
    ("ya29.", 30, Charset::Dotted, "Google OAuth access token"),
    ("dop_v1_", 64, Charset::Hex, "DigitalOcean token"),
    ("doo_v1_", 64, Charset::Hex, "DigitalOcean token"),
    ("dor_v1_", 64, Charset::Hex, "DigitalOcean token"),
    ("hvs.", 20, Charset::Base64Url, "HashiCorp Vault token"),
    ("hvb.", 20, Charset::Base64Url, "HashiCorp Vault token"),
    ("tskey-", 20, Charset::Base64Url, "Tailscale key"),
    ("npm_", 36, Charset::Base64Url, "npm token"),
    ("pypi-", 50, Charset::Base64Url, "PyPI token"),
    ("hf_", 30, Charset::Base64Url, "Hugging Face token"),
    ("dapi", 32, Charset::Hex, "Databricks token"),
    ("shpat_", 32, Charset::Hex, "Shopify token"),
    ("shpca_", 32, Charset::Hex, "Shopify token"),
    ("shppa_", 32, Charset::Hex, "Shopify token"),
    ("shpss_", 32, Charset::Hex, "Shopify token"),
    ("key-", 32, Charset::Hex, "Mailgun key"),
    ("SG.", 50, Charset::Dotted, "SendGrid API key"),
    ("ATATT3", 20, Charset::Base64Url, "Atlassian API token"),
    ("lin_api_", 20, Charset::Base64Url, "Linear API key"),
    ("ntn_", 40, Charset::Base64Url, "Notion token"),
    ("AGE-SECRET-KEY-1", 50, Charset::Base64Url, "age secret key"),
    ("SK", 32, Charset::Hex, "Twilio API key"),
];

fn classify_word(w: &str) -> Option<&'static str> {
    for (prefix, min_tail, charset, kind) in PREFIX_RULES {
        if let Some(tail) = w.strip_prefix(prefix) {
            // Random token material virtually always contains a digit;
            // requiring one keeps long identifiers that happen to share a
            // generic prefix (`sk-`, `key-`) from matching.
            if tail.len() >= *min_tail
                && charset.matches(tail)
                && tail.chars().any(|c| c.is_ascii_digit())
            {
                return Some(kind);
            }
        }
    }
    if (w.starts_with("AKIA") || w.starts_with("ASIA"))
        && w.len() == 20
        && w.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return Some("AWS access key id");
    }
    if w.starts_with("AIza") && w.len() == 39 {
        return Some("Google API key");
    }
    // Azure storage connection strings carry the account key inline.
    if let Some(tail) = w.split("AccountKey=").nth(1) {
        let key = tail.split(';').next().unwrap_or("");
        if key.len() >= 40 && Charset::Base64Url.matches(key) {
            return Some("Azure storage account key");
        }
    }
    // Telegram bot tokens: `<bot id>:AA<33 base64url chars>`.
    if let Some((id, tail)) = w.split_once(':') {
        if (8..=10).contains(&id.len())
            && id.chars().all(|c| c.is_ascii_digit())
            && tail.starts_with("AA")
            && tail.len() >= 35
            && Charset::Base64Url.matches(tail)
        {
            return Some("Telegram bot token");
        }
    }
    // Incoming-webhook URLs embed their secret in the path.
    if w.contains("hooks.slack.com/services/") {
        return Some("Slack webhook URL");
    }
    if w.contains("discord.com/api/webhooks/") {
        return Some("Discord webhook URL");
    }
    if w.starts_with("eyJ") {
        let mut parts = w.split('.');
        if let (Some(h), Some(p), Some(sig), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        {
            let b64ish = |s: &str| !s.is_empty() && Charset::Base64Url.matches(s);
            if b64ish(h) && b64ish(p) && b64ish(sig) {
                return Some("JSON Web Token");
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{MachineId, NodeId};
    use crate::infra::{local_machine, shell_node};
    use crate::lint::Severity;
    use crate::node::{NodeBuilder, Targets};
    use crate::varset::{VarKey, VarSet, VarValue};
    use infrazeug_secrets::VaultRef;
    use uuid::Uuid;

    fn report_for(infra: &Infra) -> LintReport {
        let mut report = LintReport::new();
        collect_plaintext_secrets(infra, &mut report);
        report
    }

    fn infra_with_global_var(key: &str, value: VarValue) -> Infra {
        let mut vars = VarSet::new();
        vars.insert(VarKey::new(key), value);
        Infra::new().with_global_vars(vars)
    }

    fn infra_with_node(op: ShellOp) -> Infra {
        let m = MachineId(Uuid::new_v4());
        Infra::new()
            .add_machine(local_machine(m, "ctl"))
            .unwrap()
            .add_node(shell_node(
                NodeId(Uuid::new_v4()),
                "n",
                op,
                Targets::Machine(m),
            ))
            .unwrap()
    }

    fn run_op(argv: &[&str]) -> ShellOp {
        ShellOp::run(argv.iter().map(|s| s.to_string()).collect())
    }

    fn scalar(s: &str) -> VarValue {
        VarValue::Scalar(serde_json::Value::String(s.to_string()))
    }

    #[test]
    fn plaintext_password_var_is_an_error() {
        let infra = infra_with_global_var("db_password", scalar("hunter2-but-longer"));
        let report = report_for(&infra);
        assert!(report.has_errors());
        assert_eq!(report.errors().next().unwrap().code(), "plaintext-secret");
    }

    #[test]
    fn vault_backed_password_var_is_clean() {
        let infra = infra_with_global_var(
            "db_password",
            VarValue::Vault(VaultRef::field("db", "password")),
        );
        assert!(report_for(&infra).is_empty());
    }

    #[test]
    fn key_path_and_placeholder_values_are_clean() {
        for value in [
            "/home/user/.ssh/id_ed25519",
            "${DB_PASSWORD}",
            "$DB_PASSWORD",
            "COTURN_AUTH_SECRET_PLACEHOLDER",
            "",
        ] {
            let infra = infra_with_global_var("private_key", scalar(value));
            assert!(report_for(&infra).is_empty(), "value {value:?} flagged");
        }
    }

    #[test]
    fn non_secret_key_names_are_clean() {
        for key in ["password_file", "token_url", "ssh_key_path", "monkey"] {
            let infra = infra_with_global_var(key, scalar("some-plain-value"));
            assert!(report_for(&infra).is_empty(), "key {key:?} flagged");
        }
    }

    #[test]
    fn known_token_in_argv_is_an_error() {
        let infra = infra_with_node(run_op(&["aws", "configure", "set", "AKIAIOSFODNN7EXAMPLE"]));
        let report = report_for(&infra);
        assert!(report.has_errors());
        assert!(report.errors().next().unwrap().message().contains("AWS"));
    }

    #[test]
    fn argv_secret_assignment_is_an_error() {
        let infra = infra_with_node(run_op(&["mysql", "--password=hunter2"]));
        assert!(report_for(&infra).has_errors());
    }

    #[test]
    fn plain_argv_is_clean() {
        let infra = infra_with_node(run_op(&["systemctl", "restart", "nginx"]));
        assert!(report_for(&infra).is_empty());
    }

    #[test]
    fn sshd_config_password_authentication_is_clean() {
        let infra = infra_with_node(ShellOp::write_file_bytes(
            "/etc/ssh/sshd_config",
            "PasswordAuthentication no\nPermitRootLogin prohibit-password\n",
            0o644,
        ));
        assert!(report_for(&infra).is_empty());
    }

    #[test]
    fn vault_yaml_placeholder_lines_are_clean() {
        let infra = infra_with_node(ShellOp::write_file_bytes(
            "/tmp/values.yaml",
            "secrets:\n  openrouterApiKey: \"\" # vault:litellm_openrouter_api_key\n  matrixPassword: '' # vault:hermes_matrix_password\n  ntfyToken: #vault:hermes_ntfy_token\n",
            0o600,
        ));
        let report = report_for(&infra);
        assert!(report.is_empty(), "{report}");
    }

    #[test]
    fn config_file_password_line_is_a_warning() {
        let infra = infra_with_node(ShellOp::write_file_bytes(
            "/etc/app.conf",
            "db_password = hunter2\n",
            0o600,
        ));
        let report = report_for(&infra);
        assert!(!report.has_errors());
        assert_eq!(report.warnings().count(), 1);
        assert_eq!(
            report.warnings().next().unwrap().severity,
            Severity::Warning
        );
    }

    #[test]
    fn pem_private_key_in_file_content_is_an_error() {
        let infra = infra_with_node(ShellOp::write_file_bytes(
            "/etc/app/id",
            "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----\n",
            0o600,
        ));
        assert!(report_for(&infra).has_errors());
    }

    #[test]
    fn vault_file_source_is_clean() {
        let infra = infra_with_node(ShellOp::write_file(
            "/etc/app/secret",
            FileSource::Vault {
                file: "app".into(),
                field: Some("secret".into()),
            },
            0o600,
        ));
        assert!(report_for(&infra).is_empty());
    }

    #[test]
    fn basic_auth_url_is_an_error() {
        let infra = infra_with_global_var(
            "registry_url",
            scalar("https://admin:hunter2@registry.example.com/v2"),
        );
        assert!(report_for(&infra).has_errors());
    }

    #[test]
    fn native_input_secret_field_is_an_error() {
        use serde_cbor::Value;
        let m = MachineId(Uuid::new_v4());
        let input = Value::Map(
            [
                (
                    Value::Text("region".into()),
                    Value::Text("eu-central-1".into()),
                ),
                (
                    Value::Text("api_token".into()),
                    Value::Text("not-a-vault-ref".into()),
                ),
            ]
            .into_iter()
            .collect(),
        );
        let node = NodeBuilder::native_with_input(
            NodeId(Uuid::new_v4()),
            "provider.create",
            input,
            Targets::Machine(m),
        )
        .name("cloud")
        .build();
        let infra = Infra::new()
            .add_machine(local_machine(m, "ctl"))
            .unwrap()
            .add_node(node)
            .unwrap();
        let report = report_for(&infra);
        assert!(report.has_errors());
        assert!(report
            .errors()
            .next()
            .unwrap()
            .message()
            .contains("api_token"));
    }

    #[test]
    fn native_input_field_name_suffix_is_clean() {
        use serde_cbor::Value;
        let m = MachineId(Uuid::new_v4());
        let input = Value::Map(
            [(
                Value::Text("password_field".into()),
                Value::Text("password".into()),
            )]
            .into_iter()
            .collect(),
        );
        let node = NodeBuilder::native_with_input(
            NodeId(Uuid::new_v4()),
            "provider.create",
            input,
            Targets::Machine(m),
        )
        .name("cloud")
        .build();
        let infra = Infra::new()
            .add_machine(local_machine(m, "ctl"))
            .unwrap()
            .add_node(node)
            .unwrap();
        assert!(report_for(&infra).is_empty());
    }

    #[test]
    fn github_token_word_is_classified() {
        assert_eq!(
            classify_token("ghp_0123456789abcdef0123456789abcdef0123"),
            Some("GitHub token")
        );
        assert_eq!(classify_token("ghp_short"), None);
    }

    #[test]
    fn provider_tokens_are_classified() {
        let hex64 = "0123456789abcdef".repeat(4);
        let cases = [
            (format!("dop_v1_{hex64}"), "DigitalOcean token"),
            (
                "hvs.CAESIG52cmFuZG9tdG9rZW4xMjM0NTY3ODkw".to_string(),
                "HashiCorp Vault token",
            ),
            (
                "tskey-auth-kFGiAS1CNTRL-2dpMswTcE89jNo8".to_string(),
                "Tailscale key",
            ),
            (
                format!("npm_{}", "a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8"),
                "npm token",
            ),
            (
                "hf_a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7".to_string(),
                "Hugging Face token",
            ),
            (format!("dapi{}", &hex64[..32]), "Databricks token"),
            (format!("shpat_{}", &hex64[..32]), "Shopify token"),
            (format!("key-{}", &hex64[..32]), "Mailgun key"),
            (
                "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".to_string(),
                "SendGrid API key",
            ),
            (
                "sq0atp-1aB2cD3eF4gH5iJ6kL7mN8".to_string(),
                "Square access token",
            ),
            (
                "GOCSPX-1aB2cD3eF4gH5iJ6kL7mN8oP9q".to_string(),
                "Google OAuth client secret",
            ),
            (
                "ya29.a0AfH6SMB1x2y3z4-example_token.0123456789".to_string(),
                "Google OAuth access token",
            ),
            (format!("SK{}", &hex64[..32]), "Twilio API key"),
            (
                "whsec_a1B2c3D4e5F6g7H8i9J0k1L2m3N4".to_string(),
                "Stripe webhook secret",
            ),
            ("sk_test_a1B2c3D4e5F6g7H8".to_string(), "Stripe test key"),
            (
                "glrt-a1B2c3D4e5F6g7H8i9J0k1".to_string(),
                "GitLab runner token",
            ),
            (
                "ATATT3xFfGF0a1B2c3D4e5F6g7H8i9J0".to_string(),
                "Atlassian API token",
            ),
            (
                "lin_api_a1B2c3D4e5F6g7H8i9J0k1".to_string(),
                "Linear API key",
            ),
            (
                "123456789:AAEa1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6".to_string(),
                "Telegram bot token",
            ),
            (
                "https://hooks.slack.com/services/T000/B000/XXXX".to_string(),
                "Slack webhook URL",
            ),
            (
                "https://discord.com/api/webhooks/123/abc".to_string(),
                "Discord webhook URL",
            ),
            (
                format!(
                    "AccountName=x;AccountKey={}==;EndpointSuffix=core.windows.net",
                    "a1B2c3D4e5F6g7H8i9J0".repeat(3)
                ),
                "Azure storage account key",
            ),
        ];
        for (token, kind) in cases {
            assert_eq!(classify_token(&token), Some(kind), "token {token:?}");
        }
    }

    #[test]
    fn generic_prefix_identifiers_are_not_classified() {
        for word in [
            "key-value-store",
            "SKIPPED_TESTS",
            // long, but no digit in the tail — identifiers, not token material
            "sk-my-very-long-kebab-case-identifier-name-goes-here",
            "secret_manager_replication_configuration_value",
            "dapifference-engine",
            "ya29.short",
        ] {
            assert_eq!(classify_token(word), None, "word {word:?} classified");
        }
    }

    #[test]
    fn inline_env_password_is_an_error() {
        let infra = infra_with_node(
            ShellOp::run(vec!["psql".into()])
                .env("DB_PASSWORD", FileSource::bytes("hunter2-but-longer")),
        );
        assert!(report_for(&infra).has_errors());
    }

    #[test]
    fn vault_env_value_is_clean() {
        let infra = infra_with_node(ShellOp::run(vec!["psql".into()]).env(
            "DB_PASSWORD",
            FileSource::Vault {
                file: "db".into(),
                field: Some("password".into()),
            },
        ));
        assert!(report_for(&infra).is_empty());
    }

    const RANDOMISH: &str = "qZ8vN3kX7mW2pR9tY4uB6cE1dF5gH0jL";

    #[test]
    fn high_entropy_var_value_is_a_warning() {
        let infra = infra_with_global_var("cluster_seed", scalar(RANDOMISH));
        let report = report_for(&infra);
        assert!(!report.has_errors());
        assert_eq!(report.warnings().count(), 1, "{report}");
        assert!(report
            .warnings()
            .next()
            .unwrap()
            .message()
            .contains("high-entropy"));
    }

    #[test]
    fn high_entropy_argv_token_is_a_warning() {
        let infra = infra_with_node(run_op(&["register", RANDOMISH]));
        let report = report_for(&infra);
        assert!(!report.has_errors());
        assert_eq!(report.warnings().count(), 1, "{report}");
    }

    #[test]
    fn orderly_values_are_not_high_entropy() {
        for value in [
            // sha256 hex digest: 4.0 bits/char ceiling
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            // uuid
            "550e8400-e29b-41d4-a716-446655440000",
            // ssh public key blob (wire-format length prefix)
            "AAAAC3NzaC1lZDI1NTE5AAAAIIGw5hPemTBnUkFmYGmHcanqcSlnFQTRgNVarYWBG2c0",
            // long identifier, no digits
            "the-quick-brown-fox-jumps-over-the-lazy-dog",
            // image ref with digest prefix kept hex-only
            "registry.example.com/app",
        ] {
            let infra = infra_with_global_var("some_setting", scalar(value));
            let report = report_for(&infra);
            assert!(report.is_empty(), "value {value:?} flagged: {report}");
        }
    }

    #[test]
    fn certificate_in_inline_script_is_clean() {
        let script = "cat > /tmp/ca.crt <<EOF\n-----BEGIN CERTIFICATE-----\nMIIBrzCCAVagAwIBAgIUXo4mJ8aB3cD5eF7gH9jK1mN2pQ4wCgYIKoZIzj0EAwIw\n-----END CERTIFICATE-----\nEOF\n";
        let infra = infra_with_node(run_op(&["sh", "-c", script]));
        let report = report_for(&infra);
        assert!(report.is_empty(), "{report}");
    }

    #[test]
    fn random_token_outside_certificate_still_warns() {
        let script = format!(
            "-----BEGIN CERTIFICATE-----\nMIIBrzCCAVagAwIBAgIUXo4mJ8aB3cD5eF7gH9jK1mN2pQ4wCgYIKoZIzj0EAwIw\n-----END CERTIFICATE-----\nregister {RANDOMISH}\n"
        );
        let infra = infra_with_node(run_op(&["sh", "-c", &script]));
        let report = report_for(&infra);
        assert_eq!(report.warnings().count(), 1, "{report}");
    }

    #[test]
    fn shannon_entropy_orders_as_expected() {
        assert!(shannon_entropy(RANDOMISH) >= 4.5);
        assert!(shannon_entropy("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa") < 1.0);
        assert!(
            shannon_entropy("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                <= 4.0
        );
    }

    #[test]
    fn provider_credential_key_names_are_errors() {
        for key in [
            "application_key",
            "application_secret",
            "consumer_key",
            "account_key",
            "client_secret",
            "secret_access_key",
        ] {
            let infra = infra_with_global_var(key, scalar("some-plain-value"));
            assert!(report_for(&infra).has_errors(), "key {key:?} not flagged");
        }
    }
}
