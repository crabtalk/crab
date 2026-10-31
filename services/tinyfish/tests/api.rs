use berm_lang::{
    http::Response,
    serde_json::{self, json},
    test,
};
#[cfg(feature = "tools")]
use tinyfish::tools;
use tinyfish::{
    api,
    types::{AgentOptions, FetchOptions, Format, SearchOptions},
};

fn sent_body(index: usize) -> serde_json::Value {
    serde_json::from_slice(&test::fetched()[index].body).unwrap()
}

#[test]
fn searches_select_their_source_and_preserve_filters_and_paper_metadata() {
    let options = SearchOptions {
        include_domains: Some(vec!["arxiv.org".into(), "example.org".into()]),
        pub_year_min: Some(2024),
        fetch: Some(Default::default()),
        page: Some(2),
        ..Default::default()
    };
    let url = "https://api.search.tinyfish.ai/?query=rust+%26+wasm&domain_type=research_paper&fetch=%7B%7D&include_domains=arxiv.org%2Cexample.org&page=2&pub_year_min=2024";
    test::respond("GET", url, 200, br#"{"query":"rust & wasm","results":[{"position":1,"site_name":"arxiv.org","title":"Paper","snippet":"A study","url":"https://arxiv.org/abs/1","authors":["A. Author"],"year":2024,"cited_by_count":12,"fetch":{"text":"Study"}}],"total_results":1,"page":2}"#);
    let page = api::search_publications("rust & wasm", &options).unwrap();
    assert_eq!(page.page, 2);
    assert_eq!(page.results[0].authors.as_ref().unwrap(), &["A. Author"]);
    assert_eq!(page.results[0].cited_by_count, Some(12));
    assert_eq!(page.results[0].fetch.as_ref().unwrap()["text"], "Study");
    assert_eq!(
        test::fetched()[0].header("X-API-Key"),
        Some("{secret:TINYFISH_API_KEY}")
    );

    test::respond(
        "GET",
        "https://api.search.tinyfish.ai/?query=rust&domain_type=news&recency_minutes=60",
        200,
        br#"{"query":"rust","results":[],"total_results":0,"page":0}"#,
    );
    api::search_news(
        "rust",
        &SearchOptions {
            recency_minutes: Some(60),
            ..Default::default()
        },
    )
    .unwrap();
    test::respond(
        "GET",
        "https://api.search.tinyfish.ai/?query=rust&domain_type=web",
        200,
        br#"{"query":"rust","results":[],"total_results":0,"page":0}"#,
    );
    api::search("rust", &SearchOptions::default()).unwrap();
}

#[test]
fn fetch_preserves_partial_failures_structured_content_and_selector_hints() {
    test::respond("POST", "https://api.fetch.tinyfish.ai", 200, br#"{"results":[{"url":"https://example.com","final_url":"https://example.com/","format":"json","text":{"heading":"Example"},"links":["https://example.com/about"],"page_metadata":{"canonical":"https://example.com"}}],"errors":[{"url":"https://missing.example","error":"selector_not_matched","candidate_selectors":["main"]}],"request_id":"request-1"}"#);
    let result = api::fetch(
        &[
            "https://example.com".into(),
            "https://missing.example".into(),
        ],
        &FetchOptions {
            format: Some(Format::Json),
            links: Some(true),
            page_metadata: Some(true),
            include_selectors: Some(vec!["article".into()]),
            ttl: Some(0),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.results[0].text["heading"], "Example");
    assert_eq!(
        result.errors[0].candidate_selectors.as_ref().unwrap(),
        &["main"]
    );
    assert_eq!(result.request_id.as_deref(), Some("request-1"));
    assert_eq!(
        sent_body(0),
        json!({"urls":["https://example.com","https://missing.example"],"format":"json","links":true,"page_metadata":true,"include_selectors":["article"],"ttl":0})
    );
}

#[test]
fn conditional_fetch_keeps_unchanged_content_and_validators() {
    test::respond("POST", "https://api.fetch.tinyfish.ai", 200, br#"{"results":[{"url":"https://example.com","final_url":"https://example.com/","format":"markdown","text":null,"not_modified":true,"etag":"v1"}],"errors":[]}"#);
    let result = api::fetch(
        &["https://example.com".into()],
        &FetchOptions {
            if_none_match: Some("v1".into()),
            include_etag_and_last_modified: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(result.results[0].text.is_null());
    assert_eq!(result.results[0].not_modified, Some(true));
    assert_eq!(result.results[0].etag.as_deref(), Some("v1"));
    assert_eq!(sent_body(0)["if_none_match"], "v1");
}

#[test]
fn agent_lifecycle_preserves_controls_results_and_terminal_failures() {
    test::respond(
        "POST",
        "https://agent.tinyfish.ai/v1/automation/run-async",
        200,
        br#"{"run_id":"run-1","error":null}"#,
    );
    let options: AgentOptions = serde_json::from_value(json!({
        "browser_profile":"stealth", "agent_config":{"mode":"strict","cursor_style":"green-pointer","max_steps":25,"max_duration_seconds":60},
        "capture_config":{"screenshots":true,"html":true}, "proxy_config":{"enabled":true,"country_code":"US"},
        "output_schema":{"type":"object","properties":{"price":{"type":"number"}}}
    })).unwrap();
    let started = api::run("https://example.com", "Find the price", &options).unwrap();
    assert_eq!(started.run_id, "run-1");
    assert_eq!(
        sent_body(0)["agent_config"]["cursor_style"],
        "green-pointer"
    );
    assert_eq!(
        sent_body(0)["output_schema"]["properties"]["price"]["type"],
        "number"
    );
    test::respond("GET", "https://agent.tinyfish.ai/v1/runs/run-1", 200, br#"{"run_id":"run-1","status":"FAILED","num_of_steps":3,"result":null,"error":{"message":"Blocked","category":"AGENT_FAILURE"},"steps":[{"step_id":"step-1"}]}"#);
    let run = api::run_status(&started.run_id).unwrap();
    assert_eq!(run.status, "FAILED");
    assert_eq!(run.num_of_steps, Some(3));
    assert_eq!(run.error.unwrap()["message"], "Blocked");
    assert_eq!(run.steps.unwrap()[0]["step_id"], "step-1");
    test::respond(
        "POST",
        "https://agent.tinyfish.ai/v1/runs/run-1/cancel",
        200,
        br#"{"run_id":"run-1","status":"FAILED","cancelled_at":null,"message":"Already terminal"}"#,
    );
    assert_eq!(api::run_cancel("run-1").unwrap().status, "FAILED");
}

#[test]
fn step_artifacts_preserve_binary_and_html_and_encode_identifiers() {
    test::respond(
        "GET",
        "https://agent.tinyfish.ai/v1/runs/run%2F1/steps/step%3F1/screenshot",
        200,
        &[0xff, 0xd8, 0xff],
    );
    let image = api::step_screenshot("run/1", "step?1").unwrap();
    assert_eq!(image.content_type, "image/jpeg");
    assert_eq!(image.base64, "/9j/");
    test::respond(
        "GET",
        "https://agent.tinyfish.ai/v1/runs/run-1/steps/step-1/html",
        200,
        b"<main>Price: 42</main>",
    );
    assert_eq!(
        api::step_html("run-1", "step-1").unwrap(),
        "<main>Price: 42</main>"
    );
}

#[test]
fn provider_failures_keep_status_codes_request_ids_and_retry_timing() {
    test::respond_with(
        "GET",
        "https://api.search.tinyfish.ai/?query=rust&domain_type=web",
        Response {
            status: 429,
            headers: vec![
                ("Retry-After".into(), "5".into()),
                ("X-Request-ID".into(), "req-1".into()),
            ],
            body: br#"{"error":{"code":"RATE_LIMIT_EXCEEDED","message":"Slow down"}}"#.to_vec(),
        },
    );
    let error = api::search("rust", &SearchOptions::default())
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("429")
            && error.contains("RATE_LIMIT_EXCEEDED")
            && error.contains("retry-after=5")
            && error.contains("req-1"),
        "{error}"
    );
    assert_eq!(test::fetched().len(), 1);
    test::respond(
        "POST",
        "https://agent.tinyfish.ai/v1/automation/run-async",
        200,
        br#"{"run_id":null,"error":{"code":"service_busy","message":"Try later"}}"#,
    );
    assert!(
        api::run("https://example.com", "Read", &AgentOptions::default())
            .unwrap_err()
            .message
            .contains("service_busy")
    );
    test::respond(
        "GET",
        "https://agent.tinyfish.ai/v1/runs/run-1",
        502,
        b"bad gateway",
    );
    assert!(
        api::run_status("run-1")
            .unwrap_err()
            .message
            .contains("502")
    );
}

#[test]
fn invalid_inputs_fail_before_spending_requests() {
    assert!(api::search(" ", &SearchOptions::default()).is_err());
    assert!(
        api::search(
            "rust",
            &SearchOptions {
                page: Some(11),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(api::fetch(&[], &FetchOptions::default()).is_err());
    assert!(
        api::fetch(
            &vec!["https://example.com".into(); 11],
            &FetchOptions::default()
        )
        .is_err()
    );
    assert!(api::fetch(&["file:///etc/hosts".into()], &FetchOptions::default()).is_err());
    assert!(
        api::fetch(
            &["https://example.com".into(), "https://example.org".into()],
            &FetchOptions {
                if_none_match: Some("v1".into()),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(api::run_status("..").is_err());
    assert!(api::step_html("run-1", "").is_err());
    assert!(test::fetched().is_empty());
}

#[cfg(feature = "tools")]
#[test]
fn tools_dispatch_optional_search_options_batch_fetch_and_agent_controls() {
    test::respond(
        "GET",
        "https://api.search.tinyfish.ai/?query=rust&domain_type=web",
        200,
        br#"{"query":"rust","results":[],"total_results":0,"page":0}"#,
    );
    let bytes = test::call(tools::berm_tool_search, br#"{"query":"rust"}"#).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["page"],
        0
    );
    test::respond("POST", "https://api.fetch.tinyfish.ai", 200, br#"{"results":[],"errors":[{"url":"https://example.com","error":"page_not_found","status":404}]}"#);
    let bytes = test::call(
        tools::berm_tool_fetch,
        br#"{"urls":["https://example.com"]}"#,
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["errors"][0]["status"],
        404
    );
    test::respond(
        "POST",
        "https://agent.tinyfish.ai/v1/automation/run-async",
        200,
        br#"{"run_id":"run-1","error":null}"#,
    );
    test::call(tools::berm_tool_run, br#"{"url":"https://example.com","goal":"Read","options":{"capture_config":{"html":true}}}"#).unwrap();
    assert_eq!(sent_body(2)["capture_config"]["html"], true);
}
