use berm_lang::{
    http::Response,
    serde_json::{self, json},
    test,
};
use resend::Email;

fn email() -> Email {
    Email {
        from: "Product <hello@example.com>".into(),
        to: vec!["reader@example.com".into()],
        subject: "Report".into(),
        text: Some("Ready".into()),
        ..Email::default()
    }
}

#[test]
fn send_uses_host_secrets_and_an_explicit_idempotency_key() {
    test::respond(
        "POST",
        "https://api.resend.com/emails",
        200,
        br#"{"id":"mail-1"}"#,
    );
    let sent = resend::send(&email(), "report/42").unwrap();
    assert_eq!(sent.id, "mail-1");
    let request = &test::fetched()[0];
    assert_eq!(
        request.header("authorization"),
        Some("Bearer {secret:RESEND_API_KEY}")
    );
    assert_eq!(request.header("idempotency-key"), Some("report/42"));
    let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(
        body,
        json!({"from":"Product <hello@example.com>","to":["reader@example.com"],"subject":"Report","text":"Ready"})
    );
}

#[test]
fn batch_and_delivery_decode_provider_results() {
    test::respond(
        "POST",
        "https://api.resend.com/emails/batch",
        200,
        br#"{"data":[{"id":"mail-1"},{"id":"mail-2"}]}"#,
    );
    let sent = resend::send_batch(&[email(), email()], "batch/1").unwrap();
    assert_eq!(sent.data.len(), 2);
    let body: serde_json::Value = serde_json::from_slice(&test::fetched()[0].body).unwrap();
    assert_eq!(body.as_array().unwrap().len(), 2);
    test::respond("GET","https://api.resend.com/emails/mail-1",200,br#"{"id":"mail-1","from":"hello@example.com","to":["reader@example.com"],"subject":"Report","created_at":"2026-10-31","last_event":"delivered","html":null,"text":"Ready"}"#);
    assert_eq!(resend::retrieve("mail-1").unwrap().last_event, "delivered");
}

#[test]
fn provider_failures_keep_status_and_retry_timing_without_retrying() {
    test::respond_with(
        "POST",
        "https://api.resend.com/emails",
        Response {
            status: 429,
            headers: vec![("Retry-After".into(), "5".into())],
            body: br#"{"name":"rate_limit_exceeded","message":"Slow down"}"#.to_vec(),
        },
    );
    let error = resend::send(&email(), "once").unwrap_err().to_string();
    assert!(
        error.contains("429")
            && error.contains("rate_limit_exceeded")
            && error.contains("retry-after=5"),
        "{error}"
    );
    assert_eq!(test::fetched().len(), 1);
}

#[cfg(feature = "tools")]
#[test]
fn tool_dispatch_accepts_optional_fields_and_returns_the_email_id() {
    test::respond(
        "POST",
        "https://api.resend.com/emails",
        200,
        br#"{"id":"mail-1"}"#,
    );
    let bytes=test::call(resend::tools::berm_tool_send,br#"{"email":{"from":"a@example.com","to":["b@example.com"],"subject":"Hi","text":"Hello"},"idempotency_key":"greet/1"}"#).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        json!({"id":"mail-1"})
    );
}

#[test]
fn non_json_errors_and_malformed_successes_fail_clearly() {
    test::respond(
        "GET",
        "https://api.resend.com/emails/id",
        502,
        b"bad gateway",
    );
    assert!(resend::retrieve("id").unwrap_err().message.contains("502"));
    test::respond("GET", "https://api.resend.com/emails/id", 200, b"{}");
    assert!(
        resend::retrieve("id")
            .unwrap_err()
            .message
            .contains("unreadable")
    );
}
