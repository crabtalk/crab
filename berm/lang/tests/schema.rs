use berm_lang::{
    Schema,
    schema::{self, Field, Piece, Shape},
    serde_json::{self, Value, json},
};

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
