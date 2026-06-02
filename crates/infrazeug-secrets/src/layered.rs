use crate::error::{Result, SecretsError};
use crate::store::VaultStore;
use crate::vault_ref::VaultRef;
use serde::de::DeserializeOwned;
use serde_cbor::Value;

/// Manual stand-in for `#[derive(VaultStruct)]` in M4.
pub async fn load_layered<T: DeserializeOwned>(
    store: &mut VaultStore,
    refs: &[VaultRef],
) -> Result<T> {
    let mut merged = serde_json::Map::new();
    let files: Vec<String> = refs.iter().map(|r| r.file.clone()).collect();
    for reference in refs {
        let val = store.resolve_field(reference).await?;
        merge_json_value(&mut merged, cbor_to_json(val)?);
    }
    let value = serde_json::Value::Object(merged);
    serde_json::from_value(value).map_err(|e| SecretsError::MissingField {
        field: e.to_string(),
        files,
    })
}

fn cbor_to_json(v: Value) -> Result<serde_json::Value> {
    Ok(match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(b),
        Value::Integer(i) => {
            if let Ok(n) = i64::try_from(i) {
                serde_json::Value::Number(n.into())
            } else if let Ok(n) = u64::try_from(i) {
                serde_json::Value::Number(n.into())
            } else {
                // Outside JSON's exact integer range; keep the value as a string
                // rather than silently corrupting it to 0.
                serde_json::Value::String(i.to_string())
            }
        }
        Value::Text(s) => serde_json::Value::String(s),
        Value::Bytes(b) => serde_json::Value::String(hex::encode(b)),
        Value::Array(a) => {
            serde_json::Value::Array(a.into_iter().map(cbor_to_json).collect::<Result<_>>()?)
        }
        Value::Map(m) => serde_json::Value::Object(
            m.into_iter()
                .filter_map(|(k, v)| {
                    let key = match k {
                        Value::Text(s) => s,
                        _ => return None,
                    };
                    Some((key, cbor_to_json(v).ok()?))
                })
                .collect(),
        ),
        Value::Tag(_, inner) => cbor_to_json(*inner)?,
        Value::Float(f) => serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        _ => serde_json::Value::Null,
    })
}

#[cfg(test)]
mod tests {
    use super::cbor_to_json;
    use serde_cbor::Value;

    #[test]
    fn integer_above_i64_is_preserved() {
        // u64 value beyond i64::MAX must not be silently coerced to 0.
        let big = (i64::MAX as i128) + 1;
        let json = cbor_to_json(Value::Integer(big)).unwrap();
        assert_eq!(json, serde_json::json!(big as u64));
        // A normal negative integer still round-trips as a JSON number.
        let neg = cbor_to_json(Value::Integer(-42)).unwrap();
        assert_eq!(neg, serde_json::json!(-42));
    }
}

fn merge_json_value(dst: &mut serde_json::Map<String, serde_json::Value>, src: serde_json::Value) {
    let serde_json::Value::Object(src) = src else {
        return;
    };
    for (k, v) in src {
        match (dst.get(&k), v) {
            (Some(serde_json::Value::Object(a)), serde_json::Value::Object(b)) => {
                let mut m = a.clone();
                merge_json_value(&mut m, serde_json::Value::Object(b));
                dst.insert(k, serde_json::Value::Object(m));
            }
            (_, v) => {
                dst.insert(k, v);
            }
        }
    }
}
