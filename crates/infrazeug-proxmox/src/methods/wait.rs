//! Block on a Proxmox worker task UPID returned by a create call.

use infrazeug_ext_proxmox_api::{ProxmoxClient, WaitOptions};
use infrazeug_resource::{ResourceError, ResourceResult};
use std::time::Duration;

/// Default time to wait for a create task before giving up.
const DEFAULT_TASK_TIMEOUT_SECS: u64 = 600;

/// Wait for the task identified by `upid` to finish.
///
/// `timeout_secs` controls the wait:
/// - `None` — wait up to [`DEFAULT_TASK_TIMEOUT_SECS`].
/// - `Some(0)` — do not wait (fire-and-forget); return immediately.
/// - `Some(n)` — wait up to `n` seconds.
///
/// An empty `upid` (e.g. a synchronous endpoint) is treated as already complete.
pub(crate) async fn await_task(
    client: &ProxmoxClient,
    node: &str,
    upid: &str,
    timeout_secs: Option<u64>,
) -> ResourceResult<()> {
    if upid.trim().is_empty() {
        return Ok(());
    }
    let timeout = match timeout_secs {
        Some(0) => return Ok(()),
        Some(secs) => Duration::from_secs(secs),
        None => Duration::from_secs(DEFAULT_TASK_TIMEOUT_SECS),
    };
    client
        .wait_for_task_with(node, upid, WaitOptions::new().timeout(timeout))
        .await
        .map_err(ResourceError::provider)?;
    Ok(())
}
