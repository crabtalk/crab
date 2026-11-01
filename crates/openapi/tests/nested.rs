extern crate alloc;
use berm_lang::{serde_json, test};

mod bindings {
    use berm_lang::http::Client;
    const CLIENT: Client<'static> = Client {
        name: "Nested",
        headers: &[],
    };
    crab_openapi::service! {
        spec: "tests/specs/api.json",
        client: crate::bindings::CLIENT,
        module: crate::bindings,
        operations: { ping: "GET /ping" },
    }
}

#[test]
fn nested_tools_dispatch_to_their_own_generated_api() {
    test::respond("GET", "https://example.com/v1/ping", 200, br#"{"ok":true}"#);
    let bytes = test::call(bindings::tools::berm_tool_ping, br#"{"input":{}}"#).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["ok"],
        true
    );
}
