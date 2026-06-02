//! Push-mode hash relay for `WaitForHash` steps (SOUL §3.10.2).
//!
//! # Cross-machine coordination microarchitecture
//!
//! In push mode a [`PlanSlice`] may contain [`SliceStep::WaitForHash`]
//! markers representing cross-machine dependencies. The controller's
//! scheduler registers expected digests via [`HashRelay::register_wait`],
//! reports completions via [`HashRelay::report_node_completion`], and
//! blocks dependent machines with [`HashRelay::wait_for`] until the
//! expected digest appears. This enables safe concurrent application
//! across multiple remote hosts.
//!
//! See `docs/protocol.md` for the full planning protocol microarchitecture.

use crate::id::{MachineId, NodeId};
use crate::slice::{Sha256Digest, WaitId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

#[derive(Clone, Default)]
pub struct HashRelay {
    inner: Arc<Mutex<HashMap<WaitId, RelaySlot>>>,
}

#[derive(Default)]
struct RelaySlot {
    expect: Option<[u8; 32]>,
    seen: Option<[u8; 32]>,
    notify: Arc<Notify>,
}

impl HashRelay {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_wait(&self, id: WaitId, expect: Sha256Digest) {
        let mut g = self.inner.lock().await;
        let slot = g.entry(id).or_default();
        slot.expect = Some(expect.0);
        if slot.seen == Some(expect.0) {
            slot.notify.notify_waiters();
        }
    }

    pub async fn report_node_completion(
        &self,
        node_id: NodeId,
        sources: &[MachineId],
        digest: [u8; 32],
    ) {
        let mut g = self.inner.lock().await;
        for (_wait_id, slot) in g.iter_mut() {
            let _ = node_id;
            let _ = sources;
            if slot.expect == Some(digest) {
                slot.seen = Some(digest);
                slot.notify.notify_waiters();
            }
        }
    }

    pub async fn report_wait_satisfied(&self, id: WaitId, digest: [u8; 32]) {
        let mut g = self.inner.lock().await;
        if let Some(slot) = g.get_mut(&id) {
            slot.seen = Some(digest);
            slot.notify.notify_waiters();
        }
    }

    pub async fn wait_for(&self, id: WaitId, expect: Sha256Digest) -> bool {
        self.register_wait(id, expect).await;
        loop {
            {
                let g = self.inner.lock().await;
                if let Some(slot) = g.get(&id) {
                    if slot.seen == Some(expect.0) {
                        return true;
                    }
                }
            }
            let notify = {
                let g = self.inner.lock().await;
                g.get(&id)
                    .map(|s| Arc::clone(&s.notify))
                    .unwrap_or_default()
            };
            notify.notified().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn wait_unblocks_when_digest_reported() {
        let relay = HashRelay::new();
        let id = WaitId(42);
        let expect = Sha256Digest([9u8; 32]);
        let relay_bg = relay.clone();
        let waiter = tokio::spawn(async move { relay_bg.wait_for(id, expect).await });
        tokio::time::sleep(Duration::from_millis(20)).await;
        relay.report_wait_satisfied(id, expect.0).await;
        let finished = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("waiter timed out")
            .expect("join");
        assert!(finished);
    }
}
