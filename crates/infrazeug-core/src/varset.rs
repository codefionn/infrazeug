use crate::error::{CoreError, Result};
use crate::id::{GroupId, MachineId};
use crate::machine::{Group, Machine};
use infrazeug_secrets::VaultRef;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VarKey(pub String);

impl VarKey {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl fmt::Display for VarKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum VarAcl {
    #[default]
    Auto,
    Prompt,
    AutoForMachines(Vec<MachineId>),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VarSet {
    pub entries: BTreeMap<VarKey, VarEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VarEntry {
    pub value: VarValue,
    #[serde(default)]
    pub acl: VarAcl,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum VarValue {
    Scalar(Value),
    Vault(VaultRef),
    List(Vec<VarValue>),
    Map(BTreeMap<String, VarValue>),
}

impl VarSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: impl Into<VarKey>, value: VarValue) {
        self.entries.insert(
            key.into(),
            VarEntry {
                value,
                acl: VarAcl::Auto,
            },
        );
    }

    pub fn insert_with_acl(&mut self, key: impl Into<VarKey>, value: VarValue, acl: VarAcl) {
        self.entries.insert(key.into(), VarEntry { value, acl });
    }

    pub fn merge_from(&self, other: &VarSet) -> VarSet {
        let mut out = self.clone();
        for (k, e) in &other.entries {
            match out.entries.get_mut(k) {
                Some(existing) => {
                    existing.value = merge_values(&existing.value, &e.value);
                    if !matches!(e.acl, VarAcl::Auto) {
                        existing.acl = e.acl.clone();
                    }
                }
                None => {
                    out.entries.insert(k.clone(), e.clone());
                }
            }
        }
        out
    }

    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let path = path.as_ref();
        let data = std::fs::read_to_string(path)?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let map: BTreeMap<String, Value> = match ext {
            "json" => serde_json::from_str(&data)?,
            "toml" => {
                let table: toml::Table =
                    toml::from_str(&data).map_err(|e| CoreError::other(e.to_string()))?;
                table
                    .into_iter()
                    .map(|(k, v)| (k, toml_to_json(v)))
                    .collect()
            }
            _ => return Err(CoreError::other("vars file must be .json or .toml")),
        };
        let mut vs = VarSet::new();
        for (k, v) in map {
            vs.insert(VarKey::new(k), VarValue::Scalar(v));
        }
        Ok(vs)
    }

    /// Deserialize this var set into a typed struct `T`.
    ///
    /// Each top-level entry becomes a field of `T` (keys map to field names).
    /// `Vault`-backed entries are skipped — they require decryption first — so
    /// `T` must treat any vault-sourced field as `Option`/defaulted until vault
    /// resolution is wired into var loading (post-M4).
    pub fn load<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        let mut map = serde_json::Map::new();
        for (k, e) in &self.entries {
            if let Some(v) = scalar_json(&e.value) {
                map.insert(k.0.clone(), v);
            }
        }
        serde_json::from_value(Value::Object(map))
            .map_err(|e| CoreError::other(format!("typed var load failed: {e}")))
    }
}

fn toml_to_json(v: toml::Value) -> Value {
    match v {
        toml::Value::String(s) => Value::String(s),
        toml::Value::Integer(i) => Value::Number(i.into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Array(a) => Value::Array(a.into_iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => {
            Value::Object(t.into_iter().map(|(k, v)| (k, toml_to_json(v))).collect())
        }
        toml::Value::Datetime(_) => Value::String(v.to_string()),
    }
}

fn merge_values(base: &VarValue, overlay: &VarValue) -> VarValue {
    match (base, overlay) {
        (VarValue::Map(a), VarValue::Map(b)) => {
            let mut m = a.clone();
            for (k, v) in b {
                m.insert(
                    k.clone(),
                    merge_values(m.get(k).unwrap_or(&VarValue::Scalar(Value::Null)), v),
                );
            }
            VarValue::Map(m)
        }
        (_, o) => o.clone(),
    }
}

#[derive(Clone, Debug)]
pub enum VarSource {
    Global,
    Group(GroupId),
    Machine,
    LikeOverride,
}

#[derive(Clone, Debug)]
pub struct ResolvedVar {
    pub key: VarKey,
    pub value: Value,
    pub source: VarSource,
    pub origin: &'static str,
}

pub fn resolve_machine(
    global: &VarSet,
    groups: &[Group],
    machine: &Machine,
    like_override: Option<&VarSet>,
) -> BTreeMap<VarKey, ResolvedVar> {
    let mut acc: BTreeMap<VarKey, (Value, VarSource)> = BTreeMap::new();

    fn fold_level(acc: &mut BTreeMap<VarKey, (Value, VarSource)>, vs: &VarSet, source: VarSource) {
        for (k, e) in &vs.entries {
            if let Some(v) = scalar_json(&e.value) {
                acc.insert(k.clone(), (v, source.clone()));
            }
        }
    }

    fold_level(&mut acc, global, VarSource::Global);
    for gid in &machine.groups {
        if let Some(g) = groups.iter().find(|g| g.id == *gid) {
            fold_level(&mut acc, &g.vars, VarSource::Group(*gid));
        }
    }
    fold_level(&mut acc, &machine.vars, VarSource::Machine);
    if let Some(lo) = like_override {
        fold_level(&mut acc, lo, VarSource::LikeOverride);
    }

    acc.into_iter()
        .map(|(key, (value, source))| {
            (
                key.clone(),
                ResolvedVar {
                    key,
                    value,
                    source,
                    origin: "literal",
                },
            )
        })
        .collect()
}

/// Resolve a machine's effective vars (per SOUL §3.9 precedence) directly into
/// a typed struct `T`. This is what feeds the typed `&V` context into a
/// group-targeted node body (see `infrazeug-api`'s `on_group`).
///
/// `Vault`-backed values are skipped, as in [`VarSet::load`].
pub fn resolve_machine_typed<T: serde::de::DeserializeOwned>(
    global: &VarSet,
    groups: &[Group],
    machine: &Machine,
    like_override: Option<&VarSet>,
) -> Result<T> {
    let resolved = resolve_machine(global, groups, machine, like_override);
    let mut map = serde_json::Map::new();
    for (k, rv) in resolved {
        map.insert(k.0, rv.value);
    }
    serde_json::from_value(Value::Object(map))
        .map_err(|e| CoreError::other(format!("typed machine var resolution failed: {e}")))
}

fn scalar_json(v: &VarValue) -> Option<Value> {
    match v {
        VarValue::Scalar(x) => Some(x.clone()),
        VarValue::Vault(_) => None,
        VarValue::List(items) => Some(Value::Array(items.iter().filter_map(scalar_json).collect())),
        VarValue::Map(m) => Some(Value::Object(
            m.iter()
                .filter_map(|(k, v)| scalar_json(v).map(|j| (k.clone(), j)))
                .collect(),
        )),
    }
}

#[macro_export]
macro_rules! vars {
    ($($key:literal => $val:expr),* $(,)?) => {{
        let mut __vs = $crate::varset::VarSet::new();
        $(
            __vs.insert(
                $crate::varset::VarKey::new($key),
                $crate::varset::VarValue::Scalar(serde_json::json!($val)),
            );
        )*
        __vs
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{GroupId, MachineId};
    use crate::machine::{Group, Lifecycle, Machine, MachineKind};
    use serde_json::json;
    use uuid::Uuid;

    fn machine(id: MachineId, groups: Vec<GroupId>, vars: VarSet) -> Machine {
        Machine {
            id,
            name: "m".into(),
            kind: MachineKind::Local,
            vars,
            groups,
            tags: vec![],
            max_parallel_nodes: None,
            lifecycle: Lifecycle::Persistent,
            like: None,
            lazy: false,
        }
    }

    #[test]
    fn merge_maps_deep() {
        let mut a = VarSet::new();
        a.insert(
            VarKey::new("cfg"),
            VarValue::Map(BTreeMap::from([("a".into(), VarValue::Scalar(json!(1)))])),
        );
        let mut b = VarSet::new();
        b.insert(
            VarKey::new("cfg"),
            VarValue::Map(BTreeMap::from([("b".into(), VarValue::Scalar(json!(2)))])),
        );
        let merged = a.merge_from(&b);
        let VarValue::Map(m) = &merged.entries[&VarKey::new("cfg")].value else {
            panic!("expected map");
        };
        match m.get("a").unwrap() {
            VarValue::Scalar(v) => assert_eq!(v, &json!(1)),
            _ => panic!("expected scalar a"),
        }
        match m.get("b").unwrap() {
            VarValue::Scalar(v) => assert_eq!(v, &json!(2)),
            _ => panic!("expected scalar b"),
        }
    }

    #[test]
    fn resolve_machine_precedence() {
        let mut global = VarSet::new();
        global.insert(VarKey::new("port"), VarValue::Scalar(json!(80)));
        let gid = GroupId(Uuid::new_v4());
        let mut group_vars = VarSet::new();
        group_vars.insert(VarKey::new("port"), VarValue::Scalar(json!(443)));
        let group = Group {
            id: gid,
            name: "web".into(),
            vars: group_vars,
        };
        let mut machine_vars = VarSet::new();
        machine_vars.insert(VarKey::new("port"), VarValue::Scalar(json!(8080)));
        let mid = MachineId(Uuid::new_v4());
        let m = machine(mid, vec![gid], machine_vars);
        let resolved = resolve_machine(&global, &[group], &m, None);
        let port = &resolved[&VarKey::new("port")];
        assert_eq!(port.value, json!(8080));
        assert!(matches!(port.source, VarSource::Machine));
    }

    #[test]
    fn from_json_and_toml_files() {
        let dir = std::env::temp_dir().join(format!("iz-vars-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let json_path = dir.join("vars.json");
        std::fs::write(&json_path, r#"{"region":"eu"}"#).unwrap();
        let from_json = VarSet::from_file(&json_path).unwrap();
        match &from_json.entries[&VarKey::new("region")].value {
            VarValue::Scalar(v) => assert_eq!(v, &json!("eu")),
            _ => panic!("expected scalar region"),
        }

        let toml_path = dir.join("vars.toml");
        std::fs::write(&toml_path, "tier = 2\n").unwrap();
        let from_toml = VarSet::from_file(&toml_path).unwrap();
        match &from_toml.entries[&VarKey::new("tier")].value {
            VarValue::Scalar(v) => assert_eq!(v, &json!(2)),
            _ => panic!("expected scalar tier"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_into_typed_struct() {
        #[derive(serde::Deserialize, PartialEq, Debug)]
        struct Cfg {
            port: u16,
            hosts: Vec<String>,
        }
        let mut vs = VarSet::new();
        vs.insert(VarKey::new("port"), VarValue::Scalar(json!(8443)));
        vs.insert(
            VarKey::new("hosts"),
            VarValue::List(vec![
                VarValue::Scalar(json!("a")),
                VarValue::Scalar(json!("b")),
            ]),
        );
        let cfg: Cfg = vs.load().unwrap();
        assert_eq!(
            cfg,
            Cfg {
                port: 8443,
                hosts: vec!["a".into(), "b".into()],
            }
        );
    }

    #[test]
    fn resolve_machine_typed_applies_precedence() {
        #[derive(serde::Deserialize)]
        struct Cfg {
            port: u16,
        }
        let mut global = VarSet::new();
        global.insert(VarKey::new("port"), VarValue::Scalar(json!(80)));
        let gid = GroupId(Uuid::new_v4());
        let mut group_vars = VarSet::new();
        group_vars.insert(VarKey::new("port"), VarValue::Scalar(json!(443)));
        let group = Group {
            id: gid,
            name: "web".into(),
            vars: group_vars,
        };
        let mut machine_vars = VarSet::new();
        machine_vars.insert(VarKey::new("port"), VarValue::Scalar(json!(8080)));
        let m = machine(MachineId(Uuid::new_v4()), vec![gid], machine_vars);
        let cfg: Cfg = resolve_machine_typed(&global, &[group], &m, None).unwrap();
        assert_eq!(cfg.port, 8080);
    }

    #[test]
    fn vault_entries_skipped_in_resolve() {
        let mut global = VarSet::new();
        global.insert(
            VarKey::new("secret"),
            VarValue::Vault(VaultRef::file("prod/db")),
        );
        let mid = MachineId(Uuid::new_v4());
        let resolved = resolve_machine(&global, &[], &machine(mid, vec![], VarSet::new()), None);
        assert!(!resolved.contains_key(&VarKey::new("secret")));
    }
}
