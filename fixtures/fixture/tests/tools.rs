//! The tools run natively, with the SDK standing in for the host.

use berm_lang::{CallError, test};

#[test]
fn echo_answers_its_arguments() {
    let out = test::call(berm_fixture::berm_tool_echo, br#"{"query":"hi","page":2}"#).unwrap();
    assert_eq!(out, br#"{"query":"hi","page":2}"#);
}

#[test]
fn boom_reports_its_message() {
    let error = test::call(berm_fixture::berm_tool_boom, b"").unwrap_err();
    assert_eq!(error, "boom, as requested");
}

#[test]
fn probe_allocates() {
    assert_eq!(
        test::call(berm_fixture::berm_tool_probe, b"").unwrap(),
        b"[7,7]"
    );
}

#[test]
fn nest_forwards_what_the_other_program_answered() {
    test::answer("inner", "echo", Ok(br#"{"query":"pong"}"#));
    let out = test::call(berm_fixture::berm_tool_nest, br#"{"query":"hi"}"#).unwrap();
    assert_eq!(out, br#"{"query":"pong"}"#);
    test::forget();
}

/// The distinction the second bit on the wire exists for: a target that never
/// ran reads differently from one that ran and said no.
#[test]
fn nest_tells_a_refusal_from_a_failure() {
    test::answer(
        "inner",
        "echo",
        Err(CallError::Refused(
            "no program named \"inner\" is deployed".into(),
        )),
    );
    let error = test::call(berm_fixture::berm_tool_nest, br#"{"query":"hi"}"#).unwrap_err();
    assert_eq!(error, "refused: no program named \"inner\" is deployed");

    test::answer(
        "inner",
        "echo",
        Err(CallError::Failed("the target said no".into())),
    );
    let error = test::call(berm_fixture::berm_tool_nest, br#"{"query":"hi"}"#).unwrap_err();
    assert_eq!(error, "the target said no");
    test::forget();
}

#[test]
fn fetch_reads_what_the_test_arranged_and_keeps_secrets_named() {
    test::respond("POST", "http://127.0.0.1/token", 201, b"created");
    let out = test::call(
        berm_fixture::berm_tool_fetch,
        br#"{"url":"http://127.0.0.1/token","header":"Authorization: Bearer {secret:fixture}","body":"a=1"}"#,
    )
    .unwrap();
    assert_eq!(out, br#"{"status":201,"bytes":7}"#);
    let fetched = test::fetched();
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].method, "POST");
    assert_eq!(
        fetched[0].headers,
        [(
            "Authorization".to_owned(),
            "Bearer {secret:fixture}".to_owned()
        )]
    );
    assert_eq!(fetched[0].body, b"a=1");
    test::forget();
}
