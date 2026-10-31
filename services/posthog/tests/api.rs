use berm_lang::{
    http::Response,
    serde_json::{self, Value, json},
    test,
};
use posthog::{Evaluation, Event, PostHog, Region};
fn event() -> Event {
    Event {
        event: "report_ready".into(),
        distinct_id: "user-42".into(),
        properties: Some(
            serde_json::from_value(json!({"count":2,"paid":true,"nested":{"x":[null,1]}})).unwrap(),
        ),
        timestamp: Some("2026-10-31T00:00:00Z".into()),
        uuid: Some("c23a88d8-26ed-44d4-993a-d740fe95f077".into()),
    }
}
fn body() -> Value {
    serde_json::from_slice(&test::fetched().pop().unwrap().body).unwrap()
}

#[test]
fn capture_and_batch_use_ingestion_token_and_preserve_properties() {
    let client = PostHog::new(Region::Eu);
    test::respond("POST", "https://eu.i.posthog.com/i/v0/e/", 200, b"1");
    client.capture(&event()).unwrap();
    let sent = body();
    assert_eq!(sent["api_key"], "{secret:POSTHOG_PROJECT_TOKEN}");
    assert_eq!(sent["distinct_id"], "user-42");
    assert_eq!(sent["properties"]["count"], 2);
    assert_eq!(sent["properties"]["nested"], json!({"x":[null,1]}));
    assert_eq!(sent["uuid"], event().uuid.unwrap());
    assert!(test::fetched()[0].header("authorization").is_none());
    test::respond(
        "POST",
        "https://eu.i.posthog.com/batch/",
        200,
        br#"{"status":1}"#,
    );
    client.capture_batch(&[event(), event()]).unwrap();
    let batch = body();
    assert_eq!(batch["batch"].as_array().unwrap().len(), 2);
    assert_eq!(batch["api_key"], sent["api_key"]);
    assert!(batch["batch"][0].get("api_key").is_none());
}

#[test]
fn flags_preserve_variants_payloads_and_partial_failure_status() {
    test::respond("POST","https://us.i.posthog.com/flags?v=2",200,br#"{"flags":{"new-ui":{"key":"new-ui","enabled":true,"variant":"blue","metadata":{"payload":"{\"color\":\"blue\"}"}},"disabled":{"key":"disabled","enabled":false}},"errorsWhileComputingFlags":true,"requestId":"r1"}"#);
    let evaluation = Evaluation {
        distinct_id: "user-42".into(),
        groups: Some(serde_json::from_value(json!({"company":"acme"})).unwrap()),
        person_properties: Some(serde_json::from_value(json!({"paid":true})).unwrap()),
        ..Evaluation::default()
    };
    let flags = PostHog::new(Region::Us).flags(&evaluation).unwrap();
    assert!(flags.errors_while_computing_flags);
    assert_eq!(flags.request_id.as_deref(), Some("r1"));
    assert_eq!(flags.flags["new-ui"].variant.as_deref(), Some("blue"));
    assert_eq!(
        flags.flags["new-ui"].metadata.as_ref().unwrap()["payload"],
        r#"{"color":"blue"}"#
    );
    assert!(!flags.flags["disabled"].enabled);
    assert!(!flags.flags.contains_key("missing"));
    assert_eq!(body()["groups"], json!({"company":"acme"}));
    assert_eq!(test::fetched().len(), 1);
}

#[test]
fn queries_use_region_private_host_and_private_key_with_structured_results() {
    test::respond(
        "POST",
        "https://eu.posthog.com/api/projects/123/query/",
        200,
        br#"{"columns":["event","count"],"results":[["report_ready",2]],"hasMore":false}"#,
    );
    let result = PostHog::new(Region::Eu)
        .query(
            123,
            "select event, count() from events group by event limit 10",
            "Report events",
        )
        .unwrap();
    assert_eq!(result["results"], json!([["report_ready", 2]]));
    let request = test::fetched().pop().unwrap();
    assert_eq!(
        request.header("authorization"),
        Some("Bearer {secret:POSTHOG_PERSONAL_API_KEY}")
    );
    let sent = body();
    assert!(sent.get("api_key").is_none());
    assert_eq!(sent["query"]["kind"], "HogQLQuery");
    assert_eq!(sent["async"], false);
    assert_eq!(sent["name"], "Report events");
}

#[test]
fn rate_limits_and_permission_errors_are_returned_without_retry() {
    let client = PostHog::new(Region::Us);
    test::respond_with(
        "POST",
        "https://us.i.posthog.com/i/v0/e/",
        Response {
            status: 429,
            headers: vec![("Retry-After".into(), "30".into())],
            body: br#"{"detail":"rate limit"}"#.to_vec(),
        },
    );
    let error = client.capture(&event()).unwrap_err().message;
    assert!(error.contains("429"));
    assert!(error.contains("retry-after=30"));
    test::respond(
        "POST",
        "https://us.posthog.com/api/projects/1/query/",
        403,
        br#"{"detail":"query:read required"}"#,
    );
    assert!(
        client
            .query(1, "select 1", "probe")
            .unwrap_err()
            .message
            .contains("query:read")
    );
    assert_eq!(test::fetched().len(), 2);
}

#[test]
fn invalid_identity_and_malformed_flags_do_not_become_disabled_flags() {
    let client = PostHog::new(Region::Us);
    let mut invalid = event();
    invalid.distinct_id = "x".repeat(201);
    assert!(client.capture(&invalid).is_err());
    assert!(client.capture_batch(&[]).is_err());
    assert!(client.query(0, "select 1", "probe").is_err());
    assert!(test::fetched().is_empty());
    test::respond(
        "POST",
        "https://us.i.posthog.com/flags?v=2",
        200,
        br#"{"flags":{}}"#,
    );
    assert!(
        client
            .flags(&Evaluation {
                distinct_id: "u".into(),
                ..Evaluation::default()
            })
            .is_err()
    );
}

#[cfg(feature = "tools")]
#[test]
fn query_tool_reads_deployment_configuration_and_emits_json() {
    test::var("POSTHOG_REGION", "us");
    test::var("POSTHOG_PROJECT_ID", "5");
    test::respond(
        "POST",
        "https://us.posthog.com/api/projects/5/query/",
        200,
        br#"{"columns":["one"],"results":[[1]]}"#,
    );
    let result = test::call(
        posthog::tools::berm_tool_query,
        br#"{"sql":"select 1","name":"probe"}"#,
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&result).unwrap()["results"],
        json!([[1]])
    );
}
