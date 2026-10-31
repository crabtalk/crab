extern crate alloc;
use berm_lang::{
    Schema,
    http::Client,
    schema::{self, Piece},
    serde_json::{self, Value, json},
    test,
};

const CLIENT: Client<'static> = Client {
    name: "Objects",
    headers: &[],
};
crab_openapi::service! {
    spec: "tests/specs/objects.json",
    client: crate::CLIENT,
    operations: { query: "GET /query", tree: "POST /tree" },
}

#[test]
fn flat_objects_use_deep_object_and_exploded_form_encoding() {
    let input = serde_json::from_value(json!({"query":{
        "timeout":{"milliseconds":5000,"behavior":"return-partial"},
        "headers":{"X-Token":"one&two","X-Region":"eu west"}
    }}))
    .unwrap();
    test::respond(
        "GET",
        "https://example.com/query?timeout%5Bbehavior%5D=return-partial&timeout%5Bmilliseconds%5D=5000&X-Region=eu+west&X-Token=one%26two",
        200,
        b"{}",
    );
    api::query(&input).unwrap();
    assert_eq!(test::fetched().len(), 1);
}

#[test]
fn recursive_schema_keeps_a_resource_id_and_reference_siblings() {
    const PIECES: &[Piece] = &[Piece::Shape(&types::TreeInput::SHAPE)];
    const BYTES: [u8; schema::len(PIECES)] = schema::render(PIECES);
    let schema: Value = serde_json::from_slice(&BYTES).unwrap();
    let body = &schema["properties"]["body"];
    assert!(
        body["$id"]
            .as_str()
            .unwrap()
            .starts_with("urn:crab-openapi:schema:")
    );
    assert_eq!(body["properties"]["children"]["items"]["$ref"], body["$id"]);
    assert_eq!(
        body["properties"]["children"]["items"]["description"],
        "Child node"
    );
    let body = json!({"name":"root","children":[{"name":"child","children":[{"name":"leaf"}]}]});
    let input = serde_json::from_value(json!({"body":body})).unwrap();
    test::respond("POST", "https://example.com/tree", 200, b"{}");
    api::tree(&input).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&test::fetched()[0].body).unwrap(),
        body
    );
}
