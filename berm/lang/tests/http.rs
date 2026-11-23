use berm_lang::{
    http::{Client, Request, Response},
    serde_json::{self, Value, json},
    test,
};

const CLIENT: Client<'static> = Client {
    name: "Example",
    headers: &[("X-API-Key", "{secret:EXAMPLE_KEY}")],
};
const URL: &str = "https://api.example.com/items";

#[test]
fn client_keeps_request_bytes_headers_and_success_metadata() {
    test::respond_with(
        "POST",
        URL,
        Response {
            status: 201,
            headers: vec![("ETag".into(), "v1".into())],
            body: br#"{"id":7}"#.to_vec(),
        },
    );
    let response = CLIENT.request("POST", URL, br#"{"name":"Item"}"#).unwrap();
    assert_eq!(response.header("etag"), Some("v1"));
    assert_eq!(response.read::<Value>("Example").unwrap(), json!({"id":7}));
    let request = &test::fetched()[0];
    assert_eq!(request.body, br#"{"name":"Item"}"#);
    assert_eq!(request.header("X-API-Key"), Some("{secret:EXAMPLE_KEY}"));
}

#[test]
fn http_errors_keep_body_and_retry_metadata_without_retrying() {
    test::respond_with(
        "POST",
        URL,
        Response {
            status: 429,
            headers: vec![
                ("Retry-After".into(), "5".into()),
                ("X-Request-ID".into(), "r1".into()),
                ("X-RateLimit-Reset".into(), "2000".into()),
            ],
            body: br#"{"error":{"code":"limited","detail":"Try later"}}"#.to_vec(),
        },
    );
    let error = CLIENT.request("POST", URL, b"{}").unwrap_err().message;
    for expected in [
        "Example 429",
        "limited",
        "Try later",
        "retry-after=5",
        "x-request-id=r1",
        "rate-limit-reset=2000",
    ] {
        assert!(error.contains(expected), "{error}");
    }
    assert_eq!(test::fetched().len(), 1);
    test::respond("GET", URL, 502, b"bad gateway");
    assert!(
        CLIENT
            .request("GET", URL, b"")
            .unwrap_err()
            .message
            .contains("bad gateway")
    );
}

#[test]
fn json_decode_failures_and_body_level_refusals_keep_context() {
    test::respond("GET", URL, 200, b"not JSON");
    assert!(
        CLIENT
            .json::<Value>("GET", URL, b"")
            .unwrap_err()
            .message
            .contains("Example returned an unreadable response")
    );
    let response = Response {
        status: 200,
        headers: vec![],
        body: br#"{"error":"wrong type"}"#.to_vec(),
    };
    assert!(response.failure("Redis").message.contains("wrong type"));
    let error: berm_lang::Error = serde_json::from_slice::<Value>(b"invalid")
        .unwrap_err()
        .into();
    assert!(error.message.starts_with("JSON:"));
}

#[test]
fn raw_responses_validate_json_and_keep_its_representation() {
    let body = br#"{"number":1e2, "escaped":"\u0061", "array": [1, 2]}"#;
    let raw = Response {
        status: 200,
        headers: vec![],
        body: body.to_vec(),
    }
    .read_raw("Example")
    .unwrap();
    assert_eq!(raw.get().as_bytes(), body);
    assert_eq!(serde_json::to_vec(&raw).unwrap(), body);
    assert_eq!(
        Response {
            status: 204,
            headers: vec![],
            body: vec![]
        }
        .read_raw("Example")
        .unwrap()
        .get(),
        "null"
    );
    for body in [b"{broken".to_vec(), vec![0xff]] {
        let error = Response {
            status: 200,
            headers: vec![],
            body,
        }
        .read_raw("Example")
        .unwrap_err();
        assert!(
            error
                .message
                .starts_with("Example returned an unreadable response:")
        );
    }
}

#[test]
fn a_single_request_can_opt_in_to_rotating_egress() {
    let url = "https://example.com/items";
    test::respond("GET", url, 200, b"page");
    let response = Request::new("GET", url, &[], b"")
        .rotating()
        .send()
        .unwrap();
    assert_eq!(response.body, b"page");
    assert!(test::fetched().last().unwrap().rotating);
}
