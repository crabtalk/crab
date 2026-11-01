extern crate alloc;
use crate::bindings::{api, types::ArtifactInput};
use berm_lang::{http::Response, test};

mod bindings {
    use berm_lang::http::Client;
    const CLIENT: Client<'static> = Client {
        name: "Artifacts",
        headers: &[],
    };
    crab_openapi::service! {
        spec: "tests/specs/artifacts.json",
        client: crate::bindings::CLIENT,
        module: crate::bindings,
        raw_response: true,
        tools: false,
        operations: { artifact: "GET /artifact" },
    }
}

#[test]
fn raw_responses_preserve_binary_text_headers_and_empty_bodies() {
    for body in [
        vec![0xff, 0xd8, 0xff],
        b"<main>Result</main>".to_vec(),
        vec![],
    ] {
        test::respond_with(
            "GET",
            "https://example.com/artifact",
            Response {
                status: 200,
                headers: vec![("Content-Type".into(), "image/jpeg".into())],
                body: body.clone(),
            },
        );
        let response = api::artifact(&ArtifactInput {}).unwrap();
        assert_eq!(response.body, body);
        assert_eq!(response.header("content-type"), Some("image/jpeg"));
    }
}

#[test]
fn raw_responses_still_reject_http_failures_with_retry_metadata() {
    test::respond_with(
        "GET",
        "https://example.com/artifact",
        Response {
            status: 429,
            headers: vec![("Retry-After".into(), "7".into())],
            body: b"Slow down".to_vec(),
        },
    );
    let error = api::artifact(&ArtifactInput {}).unwrap_err().to_string();
    assert!(
        error.contains("429") && error.contains("Slow down") && error.contains("retry-after=7")
    );
}
