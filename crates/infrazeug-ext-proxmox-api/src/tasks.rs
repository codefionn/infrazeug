//! Worker task status (`/nodes/{node}/tasks/{upid}/status`) and blocking waits.
//!
//! Mutating calls like creating a VM or container return immediately with a task
//! UPID; the actual work runs asynchronously on the node. [`ProxmoxClient::wait_for_task`]
//! polls the task status until it stops, mapping a non-`OK` exit to an error.

use crate::client::ProxmoxClient;
use crate::error::{ProxmoxError, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::{sleep, Instant};

/// Exit status reported by Proxmox for a successfully completed task.
pub const EXIT_OK: &str = "OK";

/// Status of a Proxmox worker task.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TaskStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upid: Option<String>,
    /// Task kind, e.g. `qmcreate`, `vzcreate`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// `running` or `stopped`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Present once the task has stopped; `OK` on success, otherwise an error string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exitstatus: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
}

impl TaskStatus {
    /// Whether the task is still running.
    pub fn is_running(&self) -> bool {
        self.status.as_deref() == Some("running")
    }

    /// Whether the task has stopped (successfully or not).
    pub fn is_stopped(&self) -> bool {
        self.status.as_deref() == Some("stopped")
    }

    /// Whether the task stopped with an `OK` exit status.
    pub fn succeeded(&self) -> bool {
        self.is_stopped() && self.exitstatus.as_deref() == Some(EXIT_OK)
    }
}

/// How to poll a task while waiting for it to finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitOptions {
    /// Delay between status polls.
    pub poll_interval: Duration,
    /// Maximum total time to wait before giving up.
    pub timeout: Duration,
}

impl WaitOptions {
    /// Default options: poll every 2s, give up after 10 minutes.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Default for WaitOptions {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(2),
            timeout: Duration::from_secs(600),
        }
    }
}

impl ProxmoxClient {
    /// `GET /nodes/{node}/tasks/{upid}/status` — read a worker task's status.
    pub async fn task_status(&self, node: &str, upid: &str) -> Result<TaskStatus> {
        self.get(&format!(
            "/nodes/{}/tasks/{}/status",
            self.encode(node),
            self.encode(upid)
        ))
        .await
    }

    /// Poll a task until it stops, using [`WaitOptions::default`].
    ///
    /// Returns the final [`TaskStatus`] on success, [`ProxmoxError::Task`] when the
    /// task exits with a non-`OK` status, or [`ProxmoxError::TaskTimeout`] when the
    /// timeout elapses first.
    pub async fn wait_for_task(&self, node: &str, upid: &str) -> Result<TaskStatus> {
        self.wait_for_task_with(node, upid, WaitOptions::default())
            .await
    }

    /// Poll a task until it stops, using explicit [`WaitOptions`].
    pub async fn wait_for_task_with(
        &self,
        node: &str,
        upid: &str,
        opts: WaitOptions,
    ) -> Result<TaskStatus> {
        let deadline = Instant::now() + opts.timeout;
        loop {
            let status = self.task_status(node, upid).await?;
            if status.is_stopped() {
                if status.succeeded() {
                    return Ok(status);
                }
                return Err(ProxmoxError::Task {
                    upid: upid.to_string(),
                    exitstatus: status
                        .exitstatus
                        .unwrap_or_else(|| "unknown error".to_string()),
                });
            }
            if Instant::now() >= deadline {
                return Err(ProxmoxError::TaskTimeout {
                    upid: upid.to_string(),
                    timeout_secs: opts.timeout.as_secs(),
                });
            }
            sleep(opts.poll_interval).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_running_status() {
        let body = r#"{"upid":"UPID:pve:1","type":"qmcreate","status":"running"}"#;
        let status: TaskStatus = serde_json::from_str(body).unwrap();
        assert!(status.is_running());
        assert!(!status.is_stopped());
        assert!(!status.succeeded());
        assert_eq!(status.task_type.as_deref(), Some("qmcreate"));
    }

    #[test]
    fn parses_successful_status() {
        let body = r#"{"upid":"UPID:pve:1","status":"stopped","exitstatus":"OK"}"#;
        let status: TaskStatus = serde_json::from_str(body).unwrap();
        assert!(status.is_stopped());
        assert!(status.succeeded());
    }

    #[test]
    fn failed_status_is_not_success() {
        let body =
            r#"{"upid":"UPID:pve:1","status":"stopped","exitstatus":"unable to create VM 100"}"#;
        let status: TaskStatus = serde_json::from_str(body).unwrap();
        assert!(status.is_stopped());
        assert!(!status.succeeded());
        assert_eq!(
            status.exitstatus.as_deref(),
            Some("unable to create VM 100")
        );
    }

    #[test]
    fn wait_options_builder() {
        let opts = WaitOptions::new()
            .poll_interval(Duration::from_secs(5))
            .timeout(Duration::from_secs(120));
        assert_eq!(opts.poll_interval, Duration::from_secs(5));
        assert_eq!(opts.timeout, Duration::from_secs(120));
    }
}
