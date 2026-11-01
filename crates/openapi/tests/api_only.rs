extern crate alloc;
use berm_lang::{http::Client, serde_json::Value, test};

const CLIENT: Client<'static> = Client {
    name: "Mirror",
    headers: &[],
};

crab_openapi::service! {
    spec: "tests/specs/api.json",
    client: crate::CLIENT,
    base_url: "https://mirror.example.com/api",
    tools: cfg(any()),
    operations: { ping: "GET /ping" },
}

#[test]
fn api_only_bindings_can_override_the_document_server() {
    test::respond("GET", "https://mirror.example.com/api/ping", 204, b"");
    assert_eq!(api::ping(&types::PingInput {}).unwrap(), Value::Null);
}
