use crab_agent::format;
use serde_json::json;

#[test]
fn return_schemas_show_scalar_array_and_nullable_types() {
    for (schema, expected) in [
        (json!({"type": "boolean"}), "boolean"),
        (json!({"type": "integer"}), "integer"),
        (
            json!({"type": "array", "items": {"type": "string"}}),
            "array<string>",
        ),
        (
            json!({"anyOf": [
                {"type": "object", "title": "Entry"}, {"type": "null"}
            ]}),
            "Entry | null",
        ),
        (
            json!({"type": "array", "items": {"anyOf": [
                {"type": "boolean"}, {"type": "null"}
            ]}}),
            "array<boolean | null>",
        ),
        (
            json!({"type": "object", "additionalProperties": {"type": "integer"}}),
            "map<string, integer>",
        ),
        (json!({}), "any"),
    ] {
        assert_eq!(format::schema(&schema), expected);
    }
}

#[test]
fn step_prices_say_per_item_or_run_and_how_the_count_is_known() {
    use berm_api::workflow::graph::Graph;
    let step = |id: &str, source: serde_json::Value, mode: serde_json::Value| {
        json!({
            "id": id, "export": id, "guards": [],
            "dependencies": [{"source": source, "optional": false, "case": null}],
            "kind": "step", "mode": mode, "price": 2_000,
            "call": {"target": {"kind": "local", "tool": "add"}, "input": {}, "output": {}, "cases": []},
        })
    };
    let graph: Graph = serde_json::from_value(json!({
        "input": {}, "output": {},
        "nodes": [
            step("ask", json!({"kind": "input"}), json!({"kind": "each", "max": 5})),
            step("notify", json!({"kind": "node", "id": "ask"}), json!({"kind": "once"})),
        ],
    }))
    .unwrap();
    assert_eq!(
        format::step_price(&graph, &graph.nodes[0], 2_000),
        "$0.0020 per item, up to 5, exact from the input"
    );
    assert_eq!(
        format::step_price(&graph, &graph.nodes[1], 2_000),
        "$0.0020 per run, bounded by a step's result"
    );
}
