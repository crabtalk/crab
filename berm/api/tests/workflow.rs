use berm_api::workflow::{
    Policy, Version,
    graph::{Entry, Mode},
    run::{Prepared, Run, State},
};
use serde_json::{Value, json};

#[test]
fn v1_entry_and_complete_run_round_trip() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/workflow-entry-v1.json")).unwrap();
    let entry: Entry = serde_json::from_value(fixture.clone()).unwrap();
    assert_eq!(serde_json::to_value(&entry).unwrap(), fixture);
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/workflow-run-v1.json")).unwrap();
    let run: Run = serde_json::from_value(fixture.clone()).unwrap();
    assert_eq!(serde_json::to_value(&run).unwrap(), fixture);
    let checkpoint = &run.checkpoints["search"];
    let State::Unknown { error } = &checkpoint.state else {
        panic!("lost unknown outcome")
    };
    assert_eq!(error.status, 403);
    assert_eq!(error.body["explain"], "E0013");
    assert_eq!(error.body["recovery"]["retain"], true);
    let Prepared::Calls { calls } = checkpoint.prepared.as_ref().unwrap() else {
        panic!("lost prepared call")
    };
    assert_eq!(calls[0].operation_key.as_deref(), Some("run-1/search/0"));
}

#[test]
fn missing_or_future_versions_and_unknown_states_are_refused() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/workflow-entry-v1.json")).unwrap();
    for version in [json!(0), json!(2), Value::Null] {
        let mut value = fixture.clone();
        value["version"] = version;
        assert!(serde_json::from_value::<Entry>(value).is_err());
    }
    assert!(serde_json::from_value::<State>(json!({"state":"absent"})).is_err());
    assert_eq!(serde_json::to_value(Version::V1).unwrap(), 1);
}

#[test]
fn pending_skipped_failed_unknown_and_successful_null_are_distinct() {
    for value in [
        json!({"state":"pending"}),
        json!({"state":"skipped"}),
        json!({"state":"succeeded","value":null}),
        json!({"state":"failed","error":{"status":400,"headers":{},"body":{"error":"invalid"}}}),
        json!({"state":"unknown","error":{"status":504,"headers":{},"body":"response lost"}}),
    ] {
        let state: State = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(state).unwrap(), value);
    }
}

#[test]
fn graph_identity_sorts_objects_but_preserves_nodes_and_schema_content() {
    let mut entry: Entry =
        serde_json::from_str(include_str!("fixtures/workflow-entry-v1.json")).unwrap();
    let original = entry.digest().unwrap();
    let reordered = serde_json::to_string_pretty(&entry).unwrap();
    assert_eq!(
        serde_json::from_str::<Entry>(&reordered)
            .unwrap()
            .digest()
            .unwrap(),
        original
    );
    entry.graph.nodes.reverse();
    assert_ne!(entry.digest().unwrap(), original);
    entry.graph.nodes.reverse();
    entry.graph.input["description"] = json!("different contract");
    assert_ne!(entry.digest().unwrap(), original);
}

#[test]
fn policy_defaults_and_symbolic_call_bounds_are_explicit() {
    let policy: Policy = serde_json::from_str("{}").unwrap();
    assert_eq!(policy.retention_ms, 604_800_000);
    assert_eq!(policy.max_each, 100);
    assert_eq!(Mode::Once.maximum(), 1);
    assert_eq!(Mode::Optional.maximum(), 1);
    assert_eq!(Mode::Each { max: 12 }.maximum(), 12);
    assert_eq!(
        serde_json::from_str::<Policy>(r#"{"max_each":3}"#)
            .unwrap()
            .max_each,
        3
    );
}
