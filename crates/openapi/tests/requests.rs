extern crate alloc;
use berm_lang::{
    Schema,
    http::Client,
    schema::{self, Piece},
    serde_json::{self, Value, json},
    test,
};

const CLIENT: Client<'static> = Client {
    name: "Example",
    headers: &[("Authorization", "Bearer {secret:EXAMPLE_KEY}")],
};

crab_openapi::service! {
    spec: "tests/specs/api.json",
    client: crate::CLIENT,
    operations: {
        fetch_item: "GET /items/{itemId}",
        create_item: "POST /items",
        ping: "GET /ping",
        optional: "POST /optional",
    },
}

#[test]
fn parameters_keep_wire_names_encoding_and_authentication() {
    let expected = "https://example.com/v1/items/a%2Fb%3F%23?limit=3&tag=one+two&tag=three%26four&fields=id%2Craw,name&sort_order=newest";
    test::respond("GET", expected, 200, br#"{"data":{"id":1}}"#);
    let input = types::FetchItemInput {
        path: types::FetchItemInputPath {
            item_id: "a/b?#".into(),
        },
        query: Some(types::FetchItemInputQuery {
            limit: Some(3),
            tag: Some(vec!["one two".into(), "three&four".into()]),
            fields: Some(vec!["id,raw".into(), "name".into()]),
            sort_order: Some(json!("newest")),
        }),
        headers: Some(types::FetchItemInputHeaders {
            x_region: Some("eu".into()),
        }),
    };
    let value = api::fetch_item(&input).unwrap();
    assert_eq!(value["data"]["id"], 1);
    let request = &test::fetched()[0];
    assert_eq!(
        request.header("Authorization"),
        Some("Bearer {secret:EXAMPLE_KEY}")
    );
    assert_eq!(request.header("X-Region"), Some("eu"));
    assert_eq!(request.header("Content-Type"), None);
}

#[test]
fn json_bodies_keep_explicit_null_and_omit_absent_fields() {
    test::respond("POST", "https://example.com/v1/items", 201, br#"{"id":7}"#);
    let output = test::call(
        tools::berm_tool_create_item,
        br#"{"input":{"body":{"displayName":"Hello","note":null}}}"#,
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        json!({"id":7})
    );
    let request = &test::fetched()[0];
    assert_eq!(
        serde_json::from_slice::<Value>(&request.body).unwrap(),
        json!({"displayName":"Hello","note":null})
    );
    assert_eq!(request.header("Content-Type"), Some("application/json"));
}

#[test]
fn optional_body_distinguishes_absent_from_json_null() {
    test::respond("POST", "https://example.com/v1/optional", 200, b"{}");
    let absent = serde_json::from_value::<types::OptionalInput>(json!({})).unwrap();
    api::optional(&absent).unwrap();
    let null = serde_json::from_value::<types::OptionalInput>(json!({"body":null})).unwrap();
    api::optional(&null).unwrap();
    let requests = test::fetched();
    assert!(requests[0].body.is_empty());
    assert_eq!(requests[0].header("Content-Type"), None);
    assert_eq!(requests[1].body, b"null");
}

#[test]
fn empty_responses_and_http_failures_use_existing_transport() {
    test::respond("GET", "https://example.com/v1/ping", 204, b"");
    assert_eq!(api::ping(&types::PingInput {}).unwrap(), Value::Null);
    test::respond("GET", "https://example.com/v1/ping", 429, b"slow down");
    assert!(
        api::ping(&types::PingInput {})
            .unwrap_err()
            .message
            .contains("Example 429: slow down")
    );
    assert_eq!(test::fetched().len(), 2);
}

#[test]
fn schemas_preserve_constraints_and_resolve_component_references() {
    const PIECES: &[Piece] = &[Piece::Shape(&types::FetchItemInput::SHAPE)];
    const OUT: [u8; schema::len(PIECES)] = schema::render(PIECES);
    let schema: Value = serde_json::from_slice(&OUT).unwrap();
    assert_eq!(
        schema["properties"]["query"]["properties"]["limit"]["maximum"],
        5
    );
    assert_eq!(
        schema["properties"]["headers"]["properties"]["X-Region"]["enum"],
        json!(["us", "eu"])
    );
    const BODY: &[Piece] = &[Piece::Shape(&types::CreateItemInput::SHAPE)];
    const BYTES: [u8; schema::len(BODY)] = schema::render(BODY);
    let schema: Value = serde_json::from_slice(&BYTES).unwrap();
    assert_eq!(
        schema["properties"]["body"]["properties"]["displayName"]["minLength"],
        1
    );
}

#[test]
fn optional_parameter_groups_can_be_omitted() {
    test::respond("GET", "https://example.com/v1/items/7", 200, b"{}");
    test::call(
        tools::berm_tool_fetch_item,
        br#"{"input":{"path":{"itemId":"7"}}}"#,
    )
    .unwrap();
    assert_eq!(test::fetched()[0].url, "https://example.com/v1/items/7");
}
