//! Test mode and `like` expansion (SOUL §5.3–5.4).

use crate::error::{CoreError, Result};
use crate::id::MachineId;
use crate::machine::{Lifecycle, Machine, MachineKind, OsFamily, OsHint, SshConfig};
use crate::report::TestReport;
use crate::runtime::RunMode;
use crate::varset::{VarKey, VarSet, VarValue};
use infrazeug_emulate::like::validate_like;
use infrazeug_emulate::spec::EmulatedKind;
use infrazeug_emulate::ContainerRef;
use tracing::warn;

/// Effective machine view for a run (may differ from declared infra in test mode).
#[derive(Clone, Debug)]
pub struct EffectiveMachine {
    pub id: MachineId,
    pub name: String,
    pub kind: MachineKind,
    pub like_override: Option<VarSet>,
    pub lifecycle: Lifecycle,
    pub skipped: bool,
}

pub fn expand_machines(
    machines: &[Machine],
    mode: RunMode,
    run_id: crate::id::RunId,
) -> (Vec<EffectiveMachine>, TestReport) {
    let mut report = TestReport::default();
    let mut out = Vec::new();

    for m in machines {
        match mode {
            RunMode::Apply => {
                out.push(EffectiveMachine {
                    id: m.id,
                    name: m.name.clone(),
                    kind: m.kind.clone(),
                    like_override: None,
                    lifecycle: m.lifecycle.clone(),
                    skipped: false,
                });
            }
            RunMode::Test => {
                if let Some(like) = &m.like {
                    if validate_like(like).is_err() {
                        warn!(machine = %m.name, "invalid like config");
                        report.skipped.push(m.id);
                        continue;
                    }
                    let kind = emulated_to_kind(&like.kind);
                    let like_vars = like_vars_to_varset(&like.vars);
                    out.push(EffectiveMachine {
                        id: m.id,
                        name: format!("{} (like)", m.name),
                        kind,
                        like_override: Some(like_vars),
                        lifecycle: Lifecycle::Ephemeral { owner: run_id },
                        skipped: false,
                    });
                } else {
                    warn!(
                        machine = %m.name,
                        "test mode: no `like` configured — skipping"
                    );
                    report.skipped.push(m.id);
                }
            }
        }
    }
    (out, report)
}

fn emulated_to_kind(kind: &EmulatedKind) -> MachineKind {
    match kind {
        EmulatedKind::Local => MachineKind::Local,
        EmulatedKind::Container(r) => MachineKind::Container(r.clone()),
        EmulatedKind::MicroVm { .. } => MachineKind::Remote {
            ssh: SshConfig::new("127.0.0.1:0").with_user("infrazeug"),
            os: Some(OsHint {
                family: OsFamily::Linux,
                distro: Some("emulated".into()),
                version: None,
                arch: None,
            }),
        },
    }
}

fn like_vars_to_varset(vars: &infrazeug_emulate::like::LikeVars) -> VarSet {
    let mut vs = VarSet::new();
    for (k, v) in &vars.0 {
        vs.entries.insert(
            VarKey(k.clone()),
            crate::varset::VarEntry {
                value: VarValue::Scalar(serde_json::Value::String(v.clone())),
                acl: Default::default(),
            },
        );
    }
    vs
}

pub fn lint_like_configs(machines: &[Machine]) -> Result<()> {
    let mut report = crate::lint::LintReport::new();
    collect_like_configs(machines, &mut report);
    report.into_result()
}

/// Collect every invalid `like` emulation config into `report`.
pub fn collect_like_configs(machines: &[Machine], report: &mut crate::lint::LintReport) {
    for m in machines {
        if let Some(like) = &m.like {
            if let Err(e) = validate_like(like) {
                report.error(
                    CoreError::other(format!(
                        "machine `{}` has invalid `like` config: {e}",
                        m.name
                    )),
                    "fix the emulation backend settings on this machine's `like`".to_string(),
                );
            }
        }
    }
}

pub fn specs_from_machines(
    machines: &[Machine],
) -> Vec<std::sync::Arc<infrazeug_emulate::ContainerSpec>> {
    use infrazeug_emulate::graph::collect_specs_from_ref;
    use infrazeug_emulate::ContainerRef;
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for m in machines {
        if let MachineKind::Container(ContainerRef::Spec(spec)) = &m.kind {
            collect_specs_from_ref(&mut seen, &mut out, spec);
        }
        if let Some(like) = &m.like {
            if let EmulatedKind::Container(ContainerRef::Spec(spec)) = &like.kind {
                collect_specs_from_ref(&mut seen, &mut out, spec);
            }
        }
    }
    out
}

pub fn specs_from_effective(
    effective: &[EffectiveMachine],
) -> Vec<std::sync::Arc<infrazeug_emulate::ContainerSpec>> {
    use infrazeug_emulate::graph::collect_specs_from_ref;
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for em in effective {
        if let MachineKind::Container(ContainerRef::Spec(spec)) = &em.kind {
            collect_specs_from_ref(&mut seen, &mut out, spec);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{MachineId, RunId};
    use crate::machine::{Lifecycle, Machine, MachineKind};
    use crate::runtime::RunMode;
    use infrazeug_emulate::like::LikeVars;
    use infrazeug_emulate::spec::{EmulatedKind, LikeConfig};
    use uuid::Uuid;

    #[test]
    fn expand_apply_keeps_declared_machines() {
        let mid = MachineId(Uuid::new_v4());
        let machines = vec![Machine {
            id: mid,
            name: "prod".into(),
            kind: MachineKind::Local,
            vars: VarSet::new(),
            groups: vec![],
            tags: vec![],
            max_parallel_nodes: None,
            lifecycle: Lifecycle::Persistent,
            like: None,
            lazy: false,
        }];
        let (effective, report) = expand_machines(&machines, RunMode::Apply, RunId(Uuid::new_v4()));
        assert_eq!(effective.len(), 1);
        assert_eq!(effective[0].id, mid);
        assert!(report.skipped.is_empty());
    }

    #[test]
    fn expand_test_skips_without_like() {
        let mid = MachineId(Uuid::new_v4());
        let machines = vec![Machine {
            id: mid,
            name: "bare".into(),
            kind: MachineKind::Local,
            vars: VarSet::new(),
            groups: vec![],
            tags: vec![],
            max_parallel_nodes: None,
            lifecycle: Lifecycle::Persistent,
            like: None,
            lazy: false,
        }];
        let (effective, report) = expand_machines(&machines, RunMode::Test, RunId(Uuid::new_v4()));
        assert!(effective.is_empty());
        assert_eq!(report.skipped, vec![mid]);
    }

    #[test]
    fn expand_test_emulates_like_local() {
        let mid = MachineId(Uuid::new_v4());
        let machines = vec![Machine {
            id: mid,
            name: "svc".into(),
            kind: MachineKind::Remote {
                ssh: SshConfig::new("prod.example"),
                os: None,
            },
            vars: VarSet::new(),
            groups: vec![],
            tags: vec![],
            max_parallel_nodes: None,
            lifecycle: Lifecycle::Persistent,
            like: Some(LikeConfig {
                kind: EmulatedKind::Local,
                vars: LikeVars::default(),
            }),
            lazy: false,
        }];
        let (effective, _) = expand_machines(&machines, RunMode::Test, RunId(Uuid::new_v4()));
        assert_eq!(effective.len(), 1);
        assert!(matches!(effective[0].kind, MachineKind::Local));
        assert!(effective[0].like_override.is_some());
    }
}
