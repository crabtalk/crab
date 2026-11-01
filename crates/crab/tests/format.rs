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
