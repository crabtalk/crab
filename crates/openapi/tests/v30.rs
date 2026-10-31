extern crate alloc;
use berm_lang::{
    Schema,
    http::Client,
    schema::{self, Piece},
    serde_json::{self, Value, json},
    test,
};

const CLIENT: Client<'static> = Client {
    name: "Dialect",
    headers: &[],
};
crab_openapi::service! {
    spec: "tests/specs/v30.json",
    client: crate::CLIENT,
    tools: false,
    operations: { echo: "POST /echo" },
}

#[test]
fn v30_schemas_preserve_nullable_bounds_reference_rules_and_literal_examples() {
    const PIECES: &[Piece] = &[Piece::Shape(&types::EchoInput::SHAPE)];
    const BYTES: [u8; schema::len(PIECES)] = schema::render(PIECES);
    let schema: Value = serde_json::from_slice(&BYTES).unwrap();
    let body = &schema["properties"]["body"]["properties"];
    assert_eq!(body["name"]["type"], json!(["string", "null"]));
    assert_eq!(body["label"]["minLength"], 2);
    assert_eq!(body["label"]["type"], "string");
    assert_eq!(body["nullable"]["type"], "string");
    assert_eq!(
        body["literal"]["examples"][0],
        json!({"nullable":true,"exclusiveMinimum":false,"schema":{"nullable":true,"type":"string"}})
    );
    assert_eq!(
        body["literal"]["x-fixture"],
        json!({"type":"number","exclusiveMinimum":true,"nullable":true})
    );
    let ratio = &schema["properties"]["query"]["properties"]["ratio"];
    assert_eq!(ratio["exclusiveMinimum"], 0);
    assert_eq!(ratio["maximum"], 1);
    assert!(ratio.get("minimum").is_none());
    assert!(ratio.get("exclusiveMaximum").is_none());
}

#[test]
fn v30_required_nullable_fields_keep_explicit_null_on_the_wire() {
    let body = json!({"name":null,"label":"ok"});
    let input: types::EchoInput = serde_json::from_value(json!({"body":body})).unwrap();
    test::respond("POST", "https://example.com/echo", 200, b"{}");
    api::echo(&input).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&test::fetched()[0].body).unwrap(),
        body
    );
    assert!(
        serde_json::from_value::<types::EchoInput>(json!({"body":{"name":null,"label":null}}))
            .is_err()
    );
}
