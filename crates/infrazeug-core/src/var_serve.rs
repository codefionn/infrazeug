use crate::error::{CoreError, Result};
use crate::id::{MachineId, NodeId};
use crate::interactor::{Interaction, InteractionResp, Interactor};
use crate::plan::PlanDigest;
use crate::varset::{VarAcl, VarKey, VarSet};
use infrazeug_secrets::VaultStore;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ApprovalKey {
    pub digest: PlanDigest,
    pub node: NodeId,
    pub machine: MachineId,
    pub var: VarKey,
}

pub struct VarServeState {
    pub plan_digest: PlanDigest,
    pub resolved: BTreeMap<VarKey, (Value, VarAcl)>,
    pub approvals: HashSet<ApprovalKey>,
}

impl VarServeState {
    pub fn build(
        digest: PlanDigest,
        global: &VarSet,
        groups: &[crate::machine::Group],
        machine: &crate::machine::Machine,
        like_override: Option<&VarSet>,
    ) -> Self {
        let mut resolved = BTreeMap::new();
        fn fold(acc: &mut BTreeMap<VarKey, (Value, VarAcl)>, vs: &VarSet) {
            for (k, e) in &vs.entries {
                if let Some(v) = scalar_or_placeholder(&e.value) {
                    acc.insert(k.clone(), (v, e.acl.clone()));
                }
            }
        }
        fold(&mut resolved, global);
        for gid in &machine.groups {
            if let Some(g) = groups.iter().find(|g| g.id == *gid) {
                fold(&mut resolved, &g.vars);
            }
        }
        fold(&mut resolved, &machine.vars);
        if let Some(lo) = like_override {
            fold(&mut resolved, lo);
        }
        Self {
            plan_digest: digest,
            resolved,
            approvals: HashSet::new(),
        }
    }
}

fn scalar_or_placeholder(v: &crate::varset::VarValue) -> Option<Value> {
    match v {
        crate::varset::VarValue::Scalar(x) => Some(x.clone()),
        crate::varset::VarValue::Vault(_) => Some(Value::String("<vault>".into())),
        crate::varset::VarValue::List(items) => Some(Value::Array(
            items.iter().filter_map(scalar_or_placeholder).collect(),
        )),
        crate::varset::VarValue::Map(m) => Some(Value::Object(
            m.iter()
                .filter_map(|(k, v)| scalar_or_placeholder(v).map(|j| (k.clone(), j)))
                .collect(),
        )),
    }
}

pub async fn resolve_var_for_rpc(
    state: &mut VarServeState,
    vault: &Mutex<VaultStore>,
    interact: Arc<dyn Interactor>,
    node: NodeId,
    machine: MachineId,
    var: VarKey,
    merged: &VarSet,
) -> Result<Value> {
    let entry = merged
        .entries
        .get(&var)
        .ok_or_else(|| CoreError::other(format!("unknown var {var}")))?;
    enforce_acl_before_resolve(state, interact, node, machine, var.clone(), &entry.acl).await?;
    let value = match &entry.value {
        crate::varset::VarValue::Scalar(v) => v.clone(),
        crate::varset::VarValue::Vault(reference) => {
            let mut store = vault.lock().await;
            let raw = store
                .resolve_field(reference)
                .await
                .map_err(CoreError::from)?;
            cbor_value_to_json(raw)?
        }
        _ => return Err(CoreError::other(format!("var {var} is not scalar/vault"))),
    };

    Ok(value)
}

async fn enforce_acl_before_resolve(
    state: &mut VarServeState,
    interact: Arc<dyn Interactor>,
    node: NodeId,
    machine: MachineId,
    var: VarKey,
    acl: &VarAcl,
) -> Result<()> {
    match acl {
        VarAcl::Auto => Ok(()),
        VarAcl::AutoForMachines(ids) if ids.contains(&machine) => Ok(()),
        VarAcl::Prompt | VarAcl::AutoForMachines(_) => {
            let key = ApprovalKey {
                digest: state.plan_digest,
                node,
                machine,
                var: var.clone(),
            };
            if state.approvals.contains(&key) {
                return Ok(());
            }
            let resp = interact
                .ask(Interaction::ApproveVarRequest {
                    node,
                    machine,
                    var: var.clone(),
                    reason: format!("push-mode var request for {var}"),
                })
                .await?;
            match resp {
                InteractionResp::Approve => {
                    state.approvals.insert(key);
                    Ok(())
                }
                InteractionResp::Deny | InteractionResp::Cancel => {
                    Err(CoreError::other("VarDenied"))
                }
                _ => Err(CoreError::InteractionDenied("expected approve/deny".into())),
            }
        }
    }
}

fn cbor_value_to_json(v: serde_cbor::Value) -> Result<Value> {
    use serde_cbor::Value as Cbor;
    Ok(match v {
        Cbor::Null => Value::Null,
        Cbor::Bool(b) => Value::Bool(b),
        Cbor::Integer(i) => {
            let n: i64 = i.try_into().unwrap_or(0);
            Value::Number(n.into())
        }
        Cbor::Text(s) => Value::String(s),
        Cbor::Bytes(b) => Value::String(hex::encode(b)),
        Cbor::Array(a) => Value::Array(
            a.into_iter()
                .map(cbor_value_to_json)
                .collect::<Result<_>>()?,
        ),
        Cbor::Map(m) => Value::Object(
            m.into_iter()
                .filter_map(|(k, v)| {
                    let key = match k {
                        Cbor::Text(s) => s,
                        _ => return None,
                    };
                    Some((key, cbor_value_to_json(v).ok()?))
                })
                .collect(),
        ),
        Cbor::Tag(_, inner) => cbor_value_to_json(*inner)?,
        Cbor::Float(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        _ => Value::Null,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactor::{Interaction, InteractionResp};
    use crate::varset::{VarEntry, VarValue};
    use async_trait::async_trait;
    use infrazeug_secrets::{FsBackend, VaultRef};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    struct DenyInteractor {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl Interactor for DenyInteractor {
        async fn ask(&self, req: Interaction) -> Result<InteractionResp> {
            assert!(matches!(req, Interaction::ApproveVarRequest { .. }));
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(InteractionResp::Deny)
        }
    }

    #[tokio::test]
    async fn prompt_acl_denies_before_vault_decrypt() {
        let dir = tempfile::tempdir().unwrap();
        let vault = VaultStore::new(
            Arc::new(FsBackend::new(dir.path())),
            dir.path().to_path_buf(),
        );
        let vault = Mutex::new(vault);
        let interactor = Arc::new(DenyInteractor {
            calls: AtomicUsize::new(0),
        });
        let var = VarKey::new("db.password");
        let mut merged = VarSet::new();
        merged.entries.insert(
            var.clone(),
            VarEntry {
                value: VarValue::Vault(VaultRef::field("missing.vault", "password")),
                acl: VarAcl::Prompt,
            },
        );
        let mut state = VarServeState {
            plan_digest: PlanDigest([0; 32]),
            resolved: BTreeMap::new(),
            approvals: HashSet::new(),
        };

        let err = resolve_var_for_rpc(
            &mut state,
            &vault,
            interactor.clone(),
            NodeId(Uuid::new_v4()),
            MachineId(Uuid::new_v4()),
            var,
            &merged,
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("VarDenied"));
        assert_eq!(interactor.calls.load(Ordering::SeqCst), 1);
    }
}
