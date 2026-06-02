use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct GlobalLimits {
    pub max_ssh_connections: usize,
    pub max_concurrent_nodes: usize,
    pub max_concurrent_builds: usize,
    pub max_fact_gathers: usize,
    pub node_semaphore: Arc<Semaphore>,
}

impl Default for GlobalLimits {
    fn default() -> Self {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let max_concurrent_nodes = cpus * 4;
        Self {
            max_ssh_connections: 32,
            max_concurrent_nodes,
            max_concurrent_builds: cpus,
            max_fact_gathers: 16,
            node_semaphore: Arc::new(Semaphore::new(max_concurrent_nodes)),
        }
    }
}
