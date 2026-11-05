use berm_api::Manifest;
use serde_json::{self, Value, json};

fn manifest(parameters: Value, returns: Value) -> Manifest {
    serde_json::from_value(json!({
        "abi_version": 1, "tools": [{
            "name": "recall", "description": "Recall a value", "public": true,
            "parameters": parameters, "returns": returns
        }]
    }))
    .unwrap()
}

#[test]
fn nullable_results_compare_their_inner_schema_and_nullability() {
    let string = json!({"type": "string"});
    let nullable_string = json!({"anyOf": [string, {"type": "null"}]});
    let nullable_integer = json!({"anyOf": [{"type": "null"}, {"type": "integer"}]});
    let old = manifest(json!({}), nullable_string.clone());
    let changed = manifest(json!({}), nullable_integer);
    assert_eq!(
        old.breaks(&changed)[0].reason,
        "result changed from string to integer"
    );
    let nonnull = manifest(json!({}), string);
    assert!(old.breaks(&nonnull).is_empty());
    assert_eq!(nonnull.breaks(&old)[0].reason, "result may now be null");
}

#[test]
fn nullable_arguments_can_be_added_but_not_removed() {
    let string = json!({"type": "string"});
    let nullable = json!({"anyOf": [string, {"type": "null"}]});
    let old = manifest(string, json!({}));
    let new = manifest(nullable, json!({}));
    assert!(old.breaks(&new).is_empty());
    assert_eq!(new.breaks(&old)[0].reason, "arguments no longer takes null");
}

#[test]
fn nullable_objects_keep_comparing_their_required_fields() {
    let old = manifest(
        json!({}),
        json!({"anyOf": [
            {"type": "object", "properties": {"key": {"type": "string"}}, "required": ["key"]},
            {"type": "null"}
        ]}),
    );
    let new = manifest(
        json!({}),
        json!({"anyOf": [
            {"type": "object", "properties": {"key": {"type": "string"}}, "required": []},
            {"type": "null"}
        ]}),
    );
    assert_eq!(
        old.breaks(&new)[0].reason,
        "result.key is no longer always present"
    );
}

#[test]
fn automatic_upgrades_require_review_for_exposure_permissions_and_breaking_schemas() {
    let old = manifest(json!({"type":"string"}), json!({"type":"string"}));
    assert!(!old.upgrade_requires_review(&old));
    let mut exposed = old.clone();
    exposed.tools[0].anonymous = true;
    assert!(old.upgrade_requires_review(&exposed));
    let mut added = old.clone();
    let mut tool = added.tools[0].clone();
    tool.name = "new-tool".into();
    added.tools.push(tool);
    assert!(old.upgrade_requires_review(&added));
    let mut permissions = old.clone();
    permissions.deps.http.fetch.push("example.com".into());
    assert!(old.upgrade_requires_review(&permissions));
    let changed = manifest(json!({"type":"integer"}), json!({"type":"string"}));
    assert!(old.upgrade_requires_review(&changed));
}
