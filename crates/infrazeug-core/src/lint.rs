//! Plan/infra lint diagnostics (SOUL §8 — typed errors, structured output).
//!
//! [`Infra::lint`](crate::infra::Infra::lint) historically failed fast on the
//! first problem. [`LintReport`] instead collects *every* issue in one pass so
//! a user sees all of them at once, each carrying a stable [`code`](Diagnostic::code),
//! a human message, optional remediation `help`, and a [`Severity`]. Errors fail
//! the lint; warnings are advisory.

use crate::error::CoreError;
use std::fmt;

/// Whether a diagnostic blocks the plan (`Error`) or is advisory (`Warning`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => f.write_str("error"),
            Severity::Warning => f.write_str("warning"),
        }
    }
}

/// One lint finding: a typed [`CoreError`] cause plus severity and remediation.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    /// The typed cause; its `Display` is the human message and `code()` the id.
    pub error: CoreError,
    /// How to fix it, when there is concrete advice to give.
    pub help: Option<String>,
}

impl Diagnostic {
    /// Stable kebab-case identifier (e.g. `cycle`, `unknown-dependency`).
    pub fn code(&self) -> &'static str {
        self.error.code()
    }

    /// Human-readable description of the problem.
    pub fn message(&self) -> String {
        self.error.to_string()
    }
}

impl serde::Serialize for Diagnostic {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Diagnostic", 4)?;
        st.serialize_field("severity", &self.severity)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.message())?;
        st.serialize_field("help", &self.help)?;
        st.end()
    }
}

/// Collected lint findings for one [`Infra`](crate::infra::Infra).
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct LintReport {
    pub diagnostics: Vec<Diagnostic>,
}

impl LintReport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a blocking error with optional remediation `help`.
    pub fn error(&mut self, error: CoreError, help: impl Into<Option<String>>) {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            error,
            help: help.into(),
        });
    }

    /// Record an advisory warning with optional remediation `help`.
    pub fn warning(&mut self, error: CoreError, help: impl Into<Option<String>>) {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            error,
            help: help.into(),
        });
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
    }

    /// Aggregate all findings into a single [`CoreError::Lint`] if any error
    /// is present; warnings alone do not fail.
    pub fn into_result(self) -> Result<(), CoreError> {
        if self.has_errors() {
            Err(CoreError::Lint(self))
        } else {
            Ok(())
        }
    }

    /// Machine-readable view (`{ "diagnostics": [ { severity, code, message, help } ] }`).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "diagnostics": &self.diagnostics })
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(&self.to_json()).unwrap_or_default()
    }

    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(&serde_json::json!({ "diagnostics": &self.diagnostics }))
            .unwrap_or_default()
    }

    pub fn to_toml(&self) -> String {
        let val = serde_json::json!({ "diagnostics": &self.diagnostics });
        toml::to_string_pretty(&val).unwrap_or_default()
    }
}

impl fmt::Display for LintReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let errors = self.errors().count();
        let warnings = self.warnings().count();
        writeln!(f, "lint found {errors} error(s), {warnings} warning(s)")?;
        for d in &self.diagnostics {
            writeln!(f, "  {}[{}]: {}", d.severity, d.code(), d.message())?;
            if let Some(help) = &d.help {
                writeln!(f, "    help: {help}")?;
            }
        }
        Ok(())
    }
}
