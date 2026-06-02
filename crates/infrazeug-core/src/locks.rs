use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

/// Named locks acquired before node dispatch (SOUL §3.8.3).
#[derive(Clone, Default)]
pub struct LockBag {
    locks: HashMap<String, Arc<Semaphore>>,
}

impl LockBag {
    pub fn semaphore(&mut self, name: impl Into<String>) -> Arc<Semaphore> {
        let name = name.into();
        self.locks
            .entry(name)
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone()
    }

    /// Resolve and hold named locks until the returned guards are dropped.
    ///
    /// The `LockBag` mutex is held only while resolving semaphore handles, not
    /// for the duration of downstream work.
    pub async fn acquire_named(
        bag: &Arc<Mutex<Self>>,
        names: &[String],
    ) -> Vec<OwnedSemaphorePermit> {
        let mut sorted: Vec<String> = names.to_vec();
        sorted.sort();
        let sems: Vec<Arc<Semaphore>> = {
            let mut bag = bag.lock().await;
            sorted.iter().map(|n| bag.semaphore(n.clone())).collect()
        };
        let mut guards = Vec::with_capacity(sems.len());
        for sem in sems {
            if let Ok(g) = sem.acquire_owned().await {
                guards.push(g);
            }
        }
        guards
    }

    pub async fn with_lock<T>(
        &mut self,
        names: &[String],
        f: impl std::future::Future<Output = T>,
    ) -> T {
        let mut sorted: Vec<String> = names.to_vec();
        sorted.sort();
        let sems: Vec<_> = sorted.iter().map(|n| self.semaphore(n.clone())).collect();
        let mut guards = Vec::new();
        for sem in &sems {
            if let Ok(g) = sem.acquire().await {
                guards.push(g);
            }
        }
        f.await
    }
}

/// Per-machine local lock bags (SOUL §3.8).
pub type LocalLockStore = Arc<Mutex<HashMap<crate::id::MachineId, Arc<Mutex<LockBag>>>>>;

pub fn new_local_lock_store() -> LocalLockStore {
    Arc::new(Mutex::new(HashMap::new()))
}

pub async fn local_lock_bag(
    store: &LocalLockStore,
    machine_id: crate::id::MachineId,
) -> Arc<Mutex<LockBag>> {
    let mut map = store.lock().await;
    map.entry(machine_id)
        .or_insert_with(|| Arc::new(Mutex::new(LockBag::default())))
        .clone()
}
