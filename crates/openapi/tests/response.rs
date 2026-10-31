extern crate alloc;
use berm_lang::{
    Result,
    http::Client,
    serde_json::{self, Value, json},
    test,
};

const CLIENT: Client<'static> = Client {
    name: "Checked",
    headers: &[],
};
crab_openapi::service! {
    spec: "tests/specs/api.json",
    client: crate::CLIENT,
    response: crate::checked,
    operations: { ping: "GET /ping" },
}

fn checked(response: Value) -> Result<Value> {
    if response["failure"] == true {
        return Err("provider failure".into());
    }
    Ok(json!({"received":response}))
}

#[test]
fn response_hook_handles_json_empty_bodies_and_tools_after_http_checks() {
    test::respond(
        "GET",
        "https://example.com/v1/ping",
        200,
        br#"{"failure":true}"#,
    );
    assert_eq!(
        api::ping(&types::PingInput {}).unwrap_err().to_string(),
        "provider failure"
    );
    test::respond("GET", "https://example.com/v1/ping", 204, b"");
    let output = test::call(tools::berm_tool_ping, br#"{"input":{}}"#).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        json!({"received":null})
    );
    test::respond(
        "GET",
        "https://example.com/v1/ping",
        500,
        b"gateway unavailable",
    );
    assert!(
        api::ping(&types::PingInput {})
            .unwrap_err()
            .to_string()
            .contains("gateway unavailable")
    );
}
