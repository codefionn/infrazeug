use crate::error::MigrateError;
use serde_cbor::Value;
use serde_yaml::Number;
use std::collections::BTreeMap;

/// Serialize a decrypted vault map as YAML for human editing.
pub fn vault_map_to_yaml(map: &BTreeMap<String, Value>) -> Result<String, MigrateError> {
    let root = cbor_map_to_yaml(map)?;
    serde_yaml::to_string(&root).map_err(|e| MigrateError::Yaml(e.to_string()))
}

/// Parse edited YAML back into a vault map (root must be a mapping).
pub fn yaml_str_to_vault_map(yaml: &str) -> Result<BTreeMap<String, Value>, MigrateError> {
    let trimmed = yaml.trim();
    if trimmed.is_empty() {
        return Ok(BTreeMap::new());
    }
    let value: serde_yaml::Value =
        serde_yaml::from_str(trimmed).map_err(|e| MigrateError::Yaml(e.to_string()))?;
    yaml_mapping_to_vault_map(value)
}

pub fn yaml_mapping_to_vault_map(
    yaml: serde_yaml::Value,
) -> Result<BTreeMap<String, Value>, MigrateError> {
    if matches!(yaml, serde_yaml::Value::Null) {
        return Ok(BTreeMap::new());
    }
    let mapping = match yaml {
        serde_yaml::Value::Mapping(m) => m,
        _ => {
            return Err(MigrateError::Yaml(
                "ansible vault plaintext must be a YAML mapping at the root".into(),
            ));
        }
    };

    let mut out = BTreeMap::new();
    for (k, v) in mapping {
        let key = yaml_key_string(&k)?;
        out.insert(key, yaml_value_to_cbor(v)?);
    }
    Ok(out)
}

fn yaml_key_string(key: &serde_yaml::Value) -> Result<String, MigrateError> {
    match key {
        serde_yaml::Value::String(s) => Ok(s.clone()),
        serde_yaml::Value::Number(n) => Ok(n.to_string()),
        serde_yaml::Value::Bool(b) => Ok(b.to_string()),
        _ => Err(MigrateError::Yaml(
            "yaml mapping keys must be strings, numbers, or booleans".into(),
        )),
    }
}

fn yaml_value_to_cbor(value: serde_yaml::Value) -> Result<Value, MigrateError> {
    Ok(match value {
        serde_yaml::Value::Null => Value::Null,
        serde_yaml::Value::Bool(b) => Value::Bool(b),
        serde_yaml::Value::Number(n) => yaml_number_to_cbor(n)?,
        serde_yaml::Value::String(s) => Value::Text(s),
        serde_yaml::Value::Sequence(seq) => Value::Array(
            seq.into_iter()
                .map(yaml_value_to_cbor)
                .collect::<Result<_, _>>()?,
        ),
        serde_yaml::Value::Mapping(m) => {
            let mut inner = BTreeMap::new();
            for (k, v) in m {
                inner.insert(yaml_key_string(&k)?, yaml_value_to_cbor(v)?);
            }
            Value::Map(
                inner
                    .into_iter()
                    .map(|(k, v)| (Value::Text(k), v))
                    .collect(),
            )
        }
        serde_yaml::Value::Tagged(t) => yaml_value_to_cbor(t.value)?,
    })
}

fn cbor_map_to_yaml(map: &BTreeMap<String, Value>) -> Result<serde_yaml::Value, MigrateError> {
    let mut out = serde_yaml::Mapping::new();
    for (k, v) in map {
        out.insert(serde_yaml::Value::String(k.clone()), cbor_value_to_yaml(v)?);
    }
    Ok(serde_yaml::Value::Mapping(out))
}

fn cbor_value_to_yaml(value: &Value) -> Result<serde_yaml::Value, MigrateError> {
    Ok(match value {
        Value::Null => serde_yaml::Value::Null,
        Value::Bool(b) => serde_yaml::Value::Bool(*b),
        Value::Integer(i) => {
            if *i >= i64::MIN as i128 && *i <= i64::MAX as i128 {
                serde_yaml::Value::Number(Number::from(*i as i64))
            } else if *i >= 0 && *i <= u64::MAX as i128 {
                serde_yaml::Value::Number(Number::from(*i as u64))
            } else {
                return Err(MigrateError::Yaml(format!("integer {i} out of yaml range")));
            }
        }
        Value::Float(f) => {
            if !f.is_finite() {
                return Err(MigrateError::Yaml(format!("non-finite float {f}")));
            }
            serde_yaml::Value::Number(Number::from(*f))
        }
        Value::Text(s) => serde_yaml::Value::String(s.clone()),
        Value::Array(items) => serde_yaml::Value::Sequence(
            items
                .iter()
                .map(cbor_value_to_yaml)
                .collect::<Result<_, _>>()?,
        ),
        Value::Map(m) => {
            let mut inner = serde_yaml::Mapping::new();
            for (k, v) in m {
                let key = cbor_key_to_yaml(k)?;
                inner.insert(key, cbor_value_to_yaml(v)?);
            }
            serde_yaml::Value::Mapping(inner)
        }
        Value::Bytes(_) | Value::Tag(..) => Err(MigrateError::Yaml(
            "vault edit does not support CBOR bytes or tagged values; use string fields".into(),
        ))?,
        other => Err(MigrateError::Yaml(format!(
            "vault edit does not support CBOR value {other:?}"
        )))?,
    })
}

fn cbor_key_to_yaml(key: &Value) -> Result<serde_yaml::Value, MigrateError> {
    match key {
        Value::Text(s) => Ok(serde_yaml::Value::String(s.clone())),
        Value::Integer(i) if *i >= i64::MIN as i128 && *i <= i64::MAX as i128 => {
            Ok(serde_yaml::Value::Number(Number::from(*i as i64)))
        }
        Value::Bool(b) => Ok(serde_yaml::Value::Bool(*b)),
        _ => Err(MigrateError::Yaml(
            "nested map keys must be strings, integers, or booleans".into(),
        )),
    }
}

fn yaml_number_to_cbor(n: Number) -> Result<Value, MigrateError> {
    if let Some(i) = n.as_i64() {
        return Ok(Value::Integer(i as i128));
    }
    if let Some(u) = n.as_u64() {
        if u <= i64::MAX as u64 {
            return Ok(Value::Integer(u as i128));
        }
    }
    if let Some(f) = n.as_f64() {
        return Ok(Value::Float(f));
    }
    Err(MigrateError::Yaml(format!("unsupported yaml number {n}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_map_yaml_roundtrip() {
        let yaml: serde_yaml::Value = serde_yaml::from_str(
            r#"
top: plain
nested:
  child: 42
list:
  - a
  - b
"#,
        )
        .unwrap();
        let map = yaml_mapping_to_vault_map(yaml).unwrap();
        let back = yaml_str_to_vault_map(&vault_map_to_yaml(&map).unwrap()).unwrap();
        assert_eq!(map, back);
    }

    #[test]
    fn nested_mapping_roundtrip_shape() {
        let yaml: serde_yaml::Value = serde_yaml::from_str(
            r#"
top: plain
nested:
  child: 42
"#,
        )
        .unwrap();
        let map = yaml_mapping_to_vault_map(yaml).unwrap();
        assert_eq!(map.get("top"), Some(&Value::Text("plain".into())));
        let nested = map.get("nested").unwrap();
        match nested {
            Value::Map(m) => {
                let child = m
                    .iter()
                    .find(|(k, _)| matches!(k, Value::Text(s) if s == "child"))
                    .map(|(_, v)| v)
                    .unwrap();
                assert_eq!(child, &Value::Integer(42_i128));
            }
            _ => panic!("expected map"),
        }
    }
}
