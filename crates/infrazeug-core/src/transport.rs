use serde::{Deserialize, Serialize};

/// How to reach a machine at apply time (SOUL §4), independent of `MachineKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportChoice {
    Local,
    SshAgentPush,
    SshAgentless,
    PullDaemon,
}
