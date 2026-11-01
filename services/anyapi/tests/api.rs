use anyapi::{api, types};
use berm_lang::{
    http::Response,
    serde_json::{self, Value, json},
    test,
};

#[test]
fn company_lookup_preserves_source_controls_filters_and_idempotency() {
    let body = json!({
        "url": "https://www.linkedin.com/company/stripe",
        "source": ["source-id"],
        "allowFallbacks": false,
        "requireFields": ["name"]
    });
    let input: types::LinkedinCompanyInput = serde_json::from_value(json!({
        "body": body,
        "query": {"fields": "name,domain", "max_cost_usd": "0.05"},
        "headers": {"Idempotency-Key": "company-lookup-1"}
    }))
    .unwrap();
    let expected = json!({"output":{"found":true,"data":{"name":"Stripe"}},"costUsd":0.002,"items":1,"provider":"AnyAPI"});
    test::respond(
        "POST",
        "https://api.getanyapi.com/v1/run/linkedin.company?fields=name%2Cdomain&max_cost_usd=0.05",
        200,
        &serde_json::to_vec(&expected).unwrap(),
    );
    assert_eq!(api::linkedin_company(&input).unwrap(), expected);
    let request = &test::fetched()[0];
    assert_eq!(
        serde_json::from_slice::<Value>(&request.body).unwrap(),
        body
    );
    assert_eq!(request.header("X-API-Key"), Some("{secret:ANYAPI_API_KEY}"));
    assert_eq!(request.header("Idempotency-Key"), Some("company-lookup-1"));
    assert_eq!(request.header("Content-Type"), Some("application/json"));
}

#[test]
fn search_sends_the_callers_cursor_without_paging_or_retrying() {
    let input: types::TwitterSearchInput = serde_json::from_value(json!({
        "body": {"query": "from:example", "cursor": "next/page+1", "limit": 20}
    }))
    .unwrap();
    test::respond(
        "POST",
        "https://api.getanyapi.com/v1/run/twitter.search",
        200,
        br#"{"output":{"posts":[],"nextCursor":"next/page+2"},"costUsd":0.01}"#,
    );
    let result = api::twitter_search(&input).unwrap();
    assert_eq!(result["output"]["nextCursor"], "next/page+2");
    let requests = test::fetched();
    assert_eq!(requests.len(), 1);
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["cursor"], "next/page+1");
    assert_eq!(body["limit"], 20);
}

#[test]
fn not_found_preserves_the_charge_and_rate_limits_preserve_retry_metadata() {
    let input: types::TiktokProfileInput = serde_json::from_value(json!({
        "body": {"handle": "missing-example"}
    }))
    .unwrap();
    let url = "https://api.getanyapi.com/v1/run/tiktok.profile";
    test::respond(
        "POST",
        url,
        200,
        br#"{"output":{"found":false,"data":null,"reason":"not_found"},"costUsd":0.002}"#,
    );
    let result = api::tiktok_profile(&input).unwrap();
    assert_eq!(result["output"]["found"], false);
    assert_eq!(result["costUsd"], 0.002);
    test::respond_with(
        "POST",
        url,
        Response {
            status: 429,
            headers: vec![("Retry-After".into(), "30".into())],
            body: br#"{"error":"rate_limited"}"#.to_vec(),
        },
    );
    let error = api::tiktok_profile(&input).unwrap_err();
    assert!(error.message.contains("rate_limited"));
    assert!(error.message.contains("retry-after=30"));
    assert_eq!(test::fetched().len(), 2);
}
