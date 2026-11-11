//! JSON Schema shapes used in service manifests.

use serde_json::Value;
use std::collections::BTreeMap;

/// The non-null branch of an `anyOf` containing one schema and `null`.
pub fn nullable(schema: &Value) -> Option<&Value> {
    let variants = schema["anyOf"].as_array()?;
    if variants.len() != 2 || !variants.iter().any(|variant| variant["type"] == "null") {
        return None;
    }
    variants.iter().find(|variant| variant["type"] != "null")
}

/// Named branches of a string enum or an externally tagged payload enum.
pub fn variants(schema: &Value) -> Option<BTreeMap<&str, Option<&Value>>> {
    if schema["type"] == "string" {
        return Some(
            values(schema)?
                .into_iter()
                .map(|name| (name, None))
                .collect(),
        );
    }
    let mut branches = BTreeMap::new();
    for branch in schema["oneOf"].as_array()? {
        let (name, payload) = if branch["type"] == "string" {
            let names = branch["enum"].as_array()?;
            if names.len() != 1 {
                return None;
            }
            (names[0].as_str()?, None)
        } else if branch["type"] == "object" {
            let fields = branch["properties"].as_object()?;
            if fields.len() != 1 || branch["additionalProperties"] != false {
                return None;
            }
            let (name, payload) = fields.iter().next()?;
            if branch["required"] != serde_json::json!([name]) {
                return None;
            }
            (name.as_str(), Some(payload))
        } else {
            return None;
        };
        if branches.insert(name, payload).is_some() {
            return None;
        }
    }
    Some(branches)
}

/// The string values listed by an enum schema.
pub fn values(schema: &Value) -> Option<Vec<&str>> {
    Some(
        schema["enum"]
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .collect(),
    )
}
