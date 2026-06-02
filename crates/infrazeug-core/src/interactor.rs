use crate::error::{CoreError, Result};
use crate::id::{MachineId, NodeId};
use crate::passphrase_io::read_passphrase_prompt;
use crate::plan::PlanDigest;
use crate::varset::VarKey;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Interaction {
    UnlockDataKey {
        name: String,
        provider: ProviderKind,
        hint: Option<String>,
    },
    ApproveVarRequest {
        node: NodeId,
        machine: MachineId,
        var: VarKey,
        reason: String,
    },
    ConfirmDestructive {
        node: NodeId,
        machine: MachineId,
        summary: String,
    },
    /// Hidden prompt for an SSH login password or private-key passphrase
    /// (resolved once at connect time; responds with [`InteractionResp::Passphrase`]).
    SshAuthSecret {
        machine: MachineId,
        /// `true` decrypts an encrypted private key; `false` is a login password.
        key_passphrase: bool,
        hint: Option<String>,
    },
    ResolveBecomeConflict {
        node: NodeId,
        options: Vec<String>,
    },
    SignPlan {
        plan_digest: PlanDigest,
        key: SigningKeyRef,
    },
}

pub use infrazeug_secrets::ProviderKind;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InteractionResp {
    Passphrase(String),
    Approve,
    Deny,
    Cancel,
    Pick(usize),
}

#[async_trait]
pub trait Interactor: Send + Sync {
    async fn ask(&self, req: Interaction) -> Result<InteractionResp>;
}

pub struct LineInteractor;

#[async_trait]
impl Interactor for LineInteractor {
    async fn ask(&self, req: Interaction) -> Result<InteractionResp> {
        match req {
            Interaction::UnlockDataKey { name, hint, .. } => {
                let hint_suffix = hint.as_ref().map(|h| format!(" ({h})")).unwrap_or_default();
                eprintln!("Unlock data key {name:?}{hint_suffix}");
                let prompt = format!("Passphrase{hint_suffix}: ");
                let secret = read_passphrase_prompt(&prompt)?;
                Ok(InteractionResp::Passphrase(secret))
            }
            Interaction::SshAuthSecret {
                key_passphrase,
                hint,
                ..
            } => {
                let what = if key_passphrase {
                    "SSH key passphrase"
                } else {
                    "SSH password"
                };
                let hint_suffix = hint.as_ref().map(|h| format!(" ({h})")).unwrap_or_default();
                let prompt = format!("{what}{hint_suffix}: ");
                let secret = read_passphrase_prompt(&prompt)?;
                Ok(InteractionResp::Passphrase(secret))
            }
            Interaction::ApproveVarRequest { var, reason, .. } => {
                eprintln!("Approve var {}? {}", var, reason);
                eprint!("[y/N]: ");
                io::stderr().flush().ok();
                let mut line = String::new();
                io::stdin()
                    .lock()
                    .read_line(&mut line)
                    .map_err(CoreError::from)?;
                if line.trim().eq_ignore_ascii_case("y") {
                    Ok(InteractionResp::Approve)
                } else {
                    Ok(InteractionResp::Deny)
                }
            }
            Interaction::ConfirmDestructive { summary, .. } => {
                eprintln!("Confirm destructive: {summary}");
                eprint!("[y/N]: ");
                io::stderr().flush().ok();
                let mut line = String::new();
                io::stdin()
                    .lock()
                    .read_line(&mut line)
                    .map_err(CoreError::from)?;
                if line.trim().eq_ignore_ascii_case("y") {
                    Ok(InteractionResp::Approve)
                } else {
                    Ok(InteractionResp::Deny)
                }
            }
            Interaction::ResolveBecomeConflict { options, .. } => {
                for (i, o) in options.iter().enumerate() {
                    eprintln!("  [{i}] {o}");
                }
                eprint!("Pick index: ");
                io::stderr().flush().ok();
                let mut line = String::new();
                io::stdin()
                    .lock()
                    .read_line(&mut line)
                    .map_err(CoreError::from)?;
                let idx: usize = line.trim().parse().unwrap_or(0);
                Ok(InteractionResp::Pick(idx))
            }
            Interaction::SignPlan { plan_digest, key } => {
                eprintln!("Sign plan {} with key {}?", plan_digest, key.0);
                Ok(InteractionResp::Approve)
            }
        }
    }
}

pub struct AutoDenyInteractor;

#[async_trait]
impl Interactor for AutoDenyInteractor {
    async fn ask(&self, req: Interaction) -> Result<InteractionResp> {
        let msg = match &req {
            Interaction::UnlockDataKey { name, .. } => format!("unlock data key {name}"),
            Interaction::ApproveVarRequest { var, .. } => format!("approve var {var}"),
            Interaction::ConfirmDestructive { summary, .. } => {
                format!("confirm destructive: {summary}")
            }
            Interaction::SshAuthSecret { key_passphrase, .. } => {
                if *key_passphrase {
                    "ssh key passphrase".into()
                } else {
                    "ssh password".into()
                }
            }
            Interaction::ResolveBecomeConflict { .. } => "resolve become conflict".into(),
            Interaction::SignPlan { .. } => "sign plan".into(),
        };
        Err(CoreError::InteractionDenied(msg))
    }
}

/// No-op interactor for M1 runs without vault unlock.
pub struct NoPromptInteractor;

#[async_trait]
impl Interactor for NoPromptInteractor {
    async fn ask(&self, _req: Interaction) -> Result<InteractionResp> {
        Ok(InteractionResp::Approve)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SigningKeyRef(pub String);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{MachineId, NodeId};
    use uuid::Uuid;

    #[tokio::test]
    async fn no_prompt_approves() {
        let i = NoPromptInteractor;
        let resp = i
            .ask(Interaction::ApproveVarRequest {
                node: NodeId(Uuid::new_v4()),
                machine: MachineId(Uuid::new_v4()),
                var: VarKey::new("token"),
                reason: "test".into(),
            })
            .await
            .unwrap();
        assert!(matches!(resp, InteractionResp::Approve));
    }

    #[tokio::test]
    async fn auto_deny_rejects_unlock() {
        let i = AutoDenyInteractor;
        let err = i
            .ask(Interaction::UnlockDataKey {
                name: "prod".into(),
                provider: ProviderKind::Passphrase,
                hint: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, CoreError::InteractionDenied(_)));
    }
}
