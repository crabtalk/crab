//! JSON Schema shapes used in service manifests.

use serde_json::Value;

/// The non-null branch of an `anyOf` containing one schema and `null`.
pub fn nullable(schema: &Value) -> Option<&Value> {
    let variants = schema["anyOf"].as_array()?;
    if variants.len() != 2 || !variants.iter().any(|variant| variant["type"] == "null") {
        return None;
    }
    variants.iter().find(|variant| variant["type"] != "null")
}
