use berm_api::{Manifest, ToolSpec, events::Source};
use serde_json::{Value, json};

fn tool() -> ToolSpec {
    serde_json::from_value(json!({
        "name":"messages", "description":"Incoming messages", "query":true,
        "events":"message.created",
        "parameters":{
            "type":"object", "required":["agent"],
            "properties":{"agent":{"type":"integer"},"cursor":{"anyOf":[{"type":"string"},{"type":"null"}]}}
        },
        "returns":{
            "type":"object", "required":["events","cursor","has_more"],
            "properties":{
                "cursor":{"type":"string"},"has_more":{"type":"boolean"},
                "events":{"type":"array","items":{
                    "type":"object","required":["id","cursor","at","data"],
                    "properties":{"id":{"type":"string"},"cursor":{"type":"string"},"at":{"type":"integer"},"data":{"type":"object","properties":{"body":{"type":"string"}},"required":["body"]}}
                }}
            }
        }
    })).unwrap()
}

#[test]
fn discovery_exposes_typed_filters_and_payload_without_a_client_cursor() {
    let source = Source::try_from(&tool()).unwrap().qualify("alice/relay");
    let discovery = serde_json::to_value(&source.definition).unwrap();
    assert_eq!(discovery["name"], "alice/relay.message.created");
    assert_eq!(discovery["delivery"], json!(["webhook"]));
    assert_eq!(
        discovery["inputSchema"]["properties"],
        json!({"agent":{"type":"integer"}})
    );
    assert_eq!(discovery["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        discovery["payloadSchema"]["properties"]["body"]["type"],
        "string"
    );
    let args: Value =
        serde_json::from_slice(&source.arguments(json!({"agent":7}), Some("4")).unwrap()).unwrap();
    assert_eq!(args, json!({"agent":7,"cursor":"4"}));
    assert!(source.arguments(json!({}), None).is_err());
    assert!(
        source
            .arguments(json!({"agent":7,"cursor":"0"}), None)
            .is_err()
    );
    assert!(
        source
            .arguments(json!({"agent":7,"owner":"bob"}), None)
            .is_err()
    );
}

#[test]
fn malformed_or_mutating_event_hooks_are_rejected() {
    let valid = tool();
    for (pointer, replacement) in [
        ("/query", json!(false)),
        ("/webhook", json!(true)),
        ("/events", json!("contains spaces")),
        ("/parameters/properties/cursor", json!({"type":"integer"})),
        ("/parameters/required", json!(["agent", "cursor"])),
        (
            "/returns/properties/events/items/required",
            json!(["id", "cursor", "data"]),
        ),
        ("/returns/properties/cursor/type", json!("integer")),
    ] {
        let mut wire = serde_json::to_value(&valid).unwrap();
        // Absent false flags are omitted by ToolSpec's serializer.
        if pointer == "/webhook" {
            wire["webhook"] = replacement;
        } else {
            *wire.pointer_mut(pointer).unwrap() = replacement;
        }
        let invalid: ToolSpec = serde_json::from_value(wire).unwrap();
        assert!(Source::try_from(&invalid).is_err(), "{pointer}");
    }
}

#[test]
fn manifests_reject_duplicate_event_names_and_read_old_tools_without_sources() {
    let original = json!({"abi_version":berm_abi::ABI_VERSION,"tools":[tool()]});
    assert!(original.to_string().parse::<Manifest>().is_ok());
    let mut duplicate = tool();
    duplicate.name = "other".into();
    let mut changed = original.clone();
    changed["tools"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::to_value(duplicate).unwrap());
    assert!(changed.to_string().parse::<Manifest>().is_err());
    let mut legacy = original;
    legacy["tools"][0].as_object_mut().unwrap().remove("events");
    assert!(
        legacy.to_string().parse::<Manifest>().unwrap().tools[0]
            .events
            .is_none()
    );
}

#[test]
fn source_pages_reject_checkpoints_that_skip_or_repeat_events() {
    let source = Source::try_from(&tool()).unwrap();
    let valid = json!({"events":[{"id":"1","cursor":"1","at":1000,"data":{"body":"Hello"}}],"cursor":"1","has_more":false});
    assert!(source.page(&valid.to_string(), Some("0")).is_ok());
    assert!(source.page(&valid.to_string(), None).is_err());
    assert!(source.page(&valid.to_string(), Some("1")).is_err());
    let mut skipped = valid;
    skipped["cursor"] = json!("2");
    assert!(source.page(&skipped.to_string(), Some("0")).is_err());
}
