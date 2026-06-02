use crate::id::{MachineId, NodeId};
use crate::node::NodeStatus;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RunReport {
    pub entries: Vec<RunReportEntry>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TestReport {
    pub skipped: Vec<MachineId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunReportEntry {
    pub node_id: NodeId,
    pub node_name: String,
    pub machine_id: MachineId,
    pub status: NodeStatus,
    pub duration: Duration,
    pub message: Option<String>,
}

impl RunReport {
    pub fn write_file(&self, path: impl AsRef<Path>) -> crate::error::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn to_json(&self) -> crate::error::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn to_yaml(&self) -> crate::error::Result<String> {
        Ok(serde_yaml::to_string(self)?)
    }

    pub fn to_toml(&self) -> crate::error::Result<String> {
        let val = serde_json::to_value(self)?;
        Ok(toml::to_string_pretty(&val)?)
    }

    pub fn write_file_format(
        &self,
        path: impl AsRef<Path>,
        fmt: crate::output::OutputFormat,
    ) -> crate::error::Result<()> {
        let content = match fmt {
            crate::output::OutputFormat::Json => self.to_json()?,
            crate::output::OutputFormat::Yaml => self.to_yaml()?,
            crate::output::OutputFormat::Toml => self.to_toml()?,
            _ => {
                return Err(crate::error::CoreError::Other(
                    "unsupported report format".into(),
                ))
            }
        };
        std::fs::write(path, content)?;
        Ok(())
    }
}
