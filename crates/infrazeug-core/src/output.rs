use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
    Yaml,
    Toml,
    Dot,
}

static FORMATS: &[&str] = &["text", "json", "yaml", "toml", "dot"];

impl OutputFormat {
    pub fn all_names() -> &'static [&'static str] {
        FORMATS
    }

    pub fn all_name_strings() -> Vec<String> {
        FORMATS.iter().map(|s| s.to_string()).collect()
    }

    pub fn name(self) -> &'static str {
        match self {
            OutputFormat::Text => "text",
            OutputFormat::Json => "json",
            OutputFormat::Yaml => "yaml",
            OutputFormat::Toml => "toml",
            OutputFormat::Dot => "dot",
        }
    }

    pub fn is_structured(self) -> bool {
        matches!(
            self,
            OutputFormat::Json | OutputFormat::Yaml | OutputFormat::Toml
        )
    }
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "text" => Ok(OutputFormat::Text),
            "json" => Ok(OutputFormat::Json),
            "yaml" => Ok(OutputFormat::Yaml),
            "toml" => Ok(OutputFormat::Toml),
            "dot" => Ok(OutputFormat::Dot),
            _ => Err(format!(
                "unknown output format {s:?} (expected one of: {})",
                FORMATS.join(", ")
            )),
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

pub fn format_to_string<T: serde::Serialize>(
    value: &T,
    fmt: OutputFormat,
) -> Result<String, crate::error::CoreError> {
    match fmt {
        OutputFormat::Json => Ok(serde_json::to_string_pretty(value)?),
        OutputFormat::Yaml => Ok(serde_yaml::to_string(value)?),
        OutputFormat::Toml => {
            let val = serde_json::to_value(value)?;
            Ok(toml::to_string_pretty(&val)?)
        }
        _ => Err(crate::error::CoreError::Other(
            "text/dot format requires type-specific rendering".into(),
        )),
    }
}
