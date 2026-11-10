use berm_lang::{
    Schema,
    abi::events::Page,
    schema::{self, Field, Piece, Shape},
    serde_json::{self, Value, json},
};
use std::collections::BTreeMap;

const SHAPE: Shape = Shape::Object {
    title: "",
    fields: &[Field {
        name: "value",
        description: "\"Nested JSON\"",
        shape: &Value::SHAPE,
    }],
};
const PIECES: &[Piece] = &[Piece::Shape(&SHAPE)];
const BYTES: [u8; schema::len(PIECES)] = schema::render(PIECES);

#[test]
fn durable_event_pages_keep_their_typed_payload_schema() {
    const PIECES: &[Piece] = &[Piece::Shape(&Page::<Entry>::SHAPE)];
    const OUT: [u8; schema::len(PIECES)] = schema::render(PIECES);
    let rendered: Value = serde_json::from_slice(&OUT).unwrap();
    assert_eq!(
        rendered["required"],
        json!(["events", "cursor", "has_more"])
    );
    let record = &rendered["properties"]["events"]["items"];
    assert_eq!(record["required"], json!(["id", "cursor", "at", "data"]));
    assert_eq!(record["properties"]["data"]["title"], "Entry");
    assert_eq!(record["properties"]["data"]["required"], json!(["key"]));
}

#[test]
fn arbitrary_json_has_an_unconstrained_schema_with_its_description() {
    let schema: Value = serde_json::from_slice(&BYTES).unwrap();
    assert_eq!(
        schema["properties"]["value"],
        json!({"description":"Nested JSON"})
    );
    assert_eq!(schema["required"], json!(["value"]));
    const ANY: &[Piece] = &[Piece::Shape(&Value::SHAPE)];
    const OUT: [u8; schema::len(ANY)] = schema::render(ANY);
    assert_eq!(&OUT, b"{}");
}

#[test]
fn imported_schemas_retain_constraints_and_field_descriptions() {
    const IMPORTED: Shape = Shape::Json(r#"{"type":["integer","null"],"minimum":1}"#);
    const SHAPE: Shape = Shape::Object {
        title: "",
        fields: &[Field {
            name: "limit",
            description: "\"Page size\"",
            shape: &IMPORTED,
        }],
    };
    const PIECES: &[Piece] = &[Piece::Shape(&SHAPE)];
    const OUT: [u8; schema::len(PIECES)] = schema::render(PIECES);
    let rendered: Value = serde_json::from_slice(&OUT).unwrap();
    assert_eq!(rendered["properties"]["limit"]["allOf"][0]["minimum"], 1);
    assert_eq!(rendered["properties"]["limit"]["description"], "Page size");
}

#[derive(Schema)]
struct Entry {
    key: String,
    note: Option<String>,
}

#[test]
fn optional_values_are_nullable_but_optional_fields_stay_omittable() {
    const PIECES: &[Piece] = &[Piece::Shape(&<Option<Entry>>::SHAPE)];
    const OUT: [u8; schema::len(PIECES)] = schema::render(PIECES);
    let rendered: Value = serde_json::from_slice(&OUT).unwrap();
    assert_eq!(rendered["anyOf"][1], json!({"type": "null"}));
    let entry = &rendered["anyOf"][0];
    assert_eq!(entry["type"], "object");
    assert_eq!(entry["required"], json!(["key"]));
    assert_eq!(entry["properties"]["note"], json!({"type": "string"}));
    assert_eq!(serde_json::to_value(None::<Entry>).unwrap(), Value::Null);
    assert_eq!(
        serde_json::to_value(Entry {
            key: "a".into(),
            note: None
        })
        .unwrap(),
        json!({"key": "a"})
    );
}

#[test]
fn optional_array_items_and_map_values_are_nullable() {
    const ARRAY: &[Piece] = &[Piece::Shape(&<Vec<Option<bool>>>::SHAPE)];
    const ARRAY_OUT: [u8; schema::len(ARRAY)] = schema::render(ARRAY);
    let array: Value = serde_json::from_slice(&ARRAY_OUT).unwrap();
    assert_eq!(
        array,
        json!({
            "type": "array",
            "items": {"anyOf": [{"type": "boolean"}, {"type": "null"}]}
        })
    );
    const MAP: &[Piece] = &[Piece::Shape(&<BTreeMap<String, Option<Entry>>>::SHAPE)];
    const MAP_OUT: [u8; schema::len(MAP)] = schema::render(MAP);
    let map: Value = serde_json::from_slice(&MAP_OUT).unwrap();
    assert_eq!(map["additionalProperties"]["anyOf"][0]["title"], "Entry");
    assert_eq!(
        map["additionalProperties"]["anyOf"][1],
        json!({"type": "null"})
    );
}
