use berm_lang::{
    http::Response,
    serde_json::{self, Value, json},
    test,
};
use upstash::Redis;
const URL: &str = "https://example.upstash.io";
fn db() -> Redis {
    Redis::new(URL).unwrap()
}
fn respond(path: &str, value: Value) {
    test::respond(
        "POST",
        &format!("{URL}{path}"),
        200,
        &serde_json::to_vec(&value).unwrap(),
    );
}
fn command(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).into()).collect()
}

#[test]
fn strings_stay_in_post_body_and_credentials_stay_as_placeholders() {
    respond("", json!({"result":"OK"}));
    db().set("key/with spaces", "a\"&?\n", Some(60)).unwrap();
    let requests = test::fetched();
    let request = &requests[0];
    assert_eq!(request.url, URL);
    assert_eq!(
        request.header("authorization"),
        Some("Bearer {secret:UPSTASH_REDIS_REST_TOKEN}")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&request.body).unwrap(),
        json!(["SET", "key/with spaces", "a\"&?\n", "EX", "60"])
    );
    respond("", json!({"result":null}));
    assert_eq!(db().get("missing").unwrap(), None);
    respond("", json!({"result":"hello"}));
    assert_eq!(db().get("present").unwrap().as_deref(), Some("hello"));
}

#[test]
fn expiry_counters_and_deletion_preserve_redis_semantics() {
    respond("", json!({"result":-2}));
    assert_eq!(db().ttl("missing").unwrap(), -2);
    respond("", json!({"result":-1}));
    assert_eq!(db().ttl("persistent").unwrap(), -1);
    respond("", json!({"result":0}));
    assert!(!db().expire("missing", 0).unwrap());
    respond("", json!({"result":1}));
    assert!(db().expire("present", 30).unwrap());
    respond("", json!({"result":-3}));
    assert_eq!(db().increment("count", -5).unwrap(), -3);
    let request = test::fetched().pop().unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&request.body).unwrap(),
        json!(["INCRBY", "count", "-5"])
    );
    respond("", json!({"result":2}));
    assert_eq!(db().delete(&command(&["a", "b"])).unwrap(), 2);
}

#[test]
fn batches_keep_order_nulls_nested_results_and_runtime_errors() {
    let commands = [
        command(&["GET", "missing"]),
        command(&["INCR", "text"]),
        command(&["MGET", "a", "b"]),
    ];
    let body = json!([{"result":null},{"error":"ERR not an integer"},{"result":["x",null]}]);
    for path in ["/pipeline", "/multi-exec"] {
        respond(path, body.clone());
        let replies = if path == "/pipeline" {
            db().pipeline(&commands)
        } else {
            db().transaction(&commands)
        }
        .unwrap();
        assert!(replies[0].value.is_null());
        assert!(replies[0].error.is_none());
        assert_eq!(replies[1].error.as_deref(), Some("ERR not an integer"));
        assert_eq!(replies[2].value, json!(["x", null]));
        let request = test::fetched().pop().unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&request.body).unwrap(),
            serde_json::to_value(&commands).unwrap()
        );
    }
}

#[test]
fn provider_errors_and_discarded_transactions_are_not_success_or_retried() {
    respond("", json!({"error":"ERR wrong type"}));
    assert!(
        db().increment("key", 1)
            .unwrap_err()
            .message
            .contains("wrong type")
    );
    respond("/multi-exec", json!({"error":"ERR unsupported"}));
    assert!(
        db().transaction(&[command(&["BAD"])])
            .unwrap_err()
            .message
            .contains("unsupported")
    );
    test::respond_with(
        "POST",
        URL,
        Response {
            status: 429,
            headers: vec![("Retry-After".into(), "12".into())],
            body: br#"{"error":"rate limit"}"#.to_vec(),
        },
    );
    let error = db().get("key").unwrap_err();
    assert!(error.message.contains("429"));
    assert!(error.message.contains("retry-after=12"));
    assert_eq!(test::fetched().len(), 3);
}

#[test]
fn invalid_configuration_and_empty_commands_fail_before_fetch() {
    for url in [
        "http://example.upstash.io",
        "https://example.com",
        "https://example.upstash.io.evil.com",
        "https://token@example.upstash.io",
        "https://example.upstash.io/path",
        "https://example.upstash.io?token=x",
        "https://example.upstash.io:444",
    ] {
        assert!(Redis::new(url).is_err(), "{url}");
    }
    assert!(db().set("key", "v", Some(0)).is_err());
    assert!(db().command(&[]).is_err());
    assert!(db().delete(&[]).is_err());
    assert!(db().pipeline(&[vec![]]).is_err());
    assert!(db().transaction(&[]).is_err());
    assert!(test::fetched().is_empty());
}

#[test]
fn malformed_success_responses_are_errors() {
    for body in [json!({}), json!({"result":23})] {
        respond("", body);
        assert!(db().get("key").is_err());
    }
    respond("/pipeline", json!([]));
    assert!(db().pipeline(&[command(&["GET", "k"])]).is_err());
}

#[cfg(feature = "tools")]
#[test]
fn tool_uses_deployment_endpoint_and_preserves_batch_results() {
    test::var("UPSTASH_REDIS_REST_URL", URL);
    respond("/pipeline", json!([{"result":null},{"result":1}]));
    let output = test::call(
        upstash::tools::berm_tool_pipeline,
        br#"{"commands":[["GET","k"],["INCR","n"]]}"#,
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        json!([{"value":null},{"value":1}])
    );
}
