use berm_api::Manifest;
use serde_json::{self, Value, json};

#[test]
fn removing_or_renaming_a_durable_event_requires_review() {
    let mut old = manifest(json!({}), json!({}));
    old.tools[0].query = true;
    old.tools[0].events = Some("message.created".into());
    let mut next = old.clone();
    next.tools[0].events = Some("changed".into());
    assert!(old.upgrade_requires_review(&next));
    assert_eq!(
        old.breaks(&next)[0].reason,
        "event source removed or renamed"
    );
    next.tools[0].events = None;
    assert!(old.upgrade_requires_review(&next));
}

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

fn platforms(required: bool, additional: bool) -> Value {
    let mut variants = vec![json!({
        "type":"object", "properties":{"grok":{
            "type":"object", "properties":{"url":{"type":"string"}},
            "required": if required { vec!["url"] } else { vec![] }
        }}, "required":["grok"], "additionalProperties":false
    })];
    if additional {
        variants.push(json!({"type":"object","properties":{"dot":{"type":"object","properties":{}}},"required":["dot"],"additionalProperties":false}));
    }
    json!({"title":"Platform","oneOf":variants})
}

#[test]
fn payload_enums_check_added_and_removed_branches_in_both_directions() {
    let old = manifest(platforms(true, false), platforms(true, false));
    let new = manifest(platforms(true, true), platforms(true, true));
    let breaks = old.breaks(&new);
    assert_eq!(breaks.len(), 1);
    assert_eq!(breaks[0].reason, "result may now be \"dot\"");
    let breaks = new.breaks(&old);
    assert_eq!(breaks.len(), 1);
    assert_eq!(breaks[0].reason, "arguments no longer takes \"dot\"");
    assert!(old.upgrade_requires_review(&new));
}

#[test]
fn payload_enum_fields_keep_their_argument_and_result_compatibility_rules() {
    let required = manifest(platforms(true, false), platforms(true, false));
    let optional = manifest(platforms(false, false), platforms(false, false));
    assert_eq!(
        required.breaks(&optional)[0].reason,
        "result.grok.url is no longer always present"
    );
    assert_eq!(
        optional.breaks(&required)[0].reason,
        "arguments.grok.url is newly required"
    );
}

#[test]
fn adding_payload_variants_preserves_existing_unit_variant_arguments() {
    let unit = json!({"type":"string","enum":["local"]});
    let mut expanded = platforms(true, false);
    expanded["oneOf"].as_array_mut().unwrap().push(unit.clone());
    let old = manifest(unit.clone(), unit);
    let new = manifest(expanded.clone(), expanded);
    let breaks = old.breaks(&new);
    assert_eq!(breaks.len(), 1);
    assert_eq!(breaks[0].reason, "result may now be \"grok\"");
}
