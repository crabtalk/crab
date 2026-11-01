use berm_lang::{
    http::Response,
    serde_json::{self, Value, json},
    test,
};
use firecrawl::{calls, format::Format};

#[test]
fn basic_scraping_disables_pdf_parsing_and_omits_absent_options() {
    test::respond(
        "POST",
        "https://api.firecrawl.dev/v2/scrape",
        200,
        br#"{"success":true,"data":{"markdown":"Example","metadata":{"statusCode":404}}}"#,
    );
    let result = calls::scrape("https://example.com".into(), None, None, None).unwrap();
    assert_eq!(result["data"]["metadata"]["statusCode"], 404);
    let request = &test::fetched()[0];
    assert_eq!(
        request.header("Authorization"),
        Some("Bearer {secret:FIRECRAWL_API_KEY}")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&request.body).unwrap(),
        json!({"url":"https://example.com","parsers":[]})
    );
}

#[test]
fn search_and_map_preserve_their_supported_wire_options() {
    test::respond(
        "POST",
        "https://api.firecrawl.dev/v2/search",
        200,
        br#"{"success":true,"creditsUsed":2,"data":{"web":[]}}"#,
    );
    let result =
        calls::search("example".into(), 3, Some(vec!["example.com".into()]), None).unwrap();
    assert_eq!(result["creditsUsed"], 2);
    let body: Value = serde_json::from_slice(&test::fetched()[0].body).unwrap();
    assert_eq!(
        body,
        json!({"query":"example","limit":3,"includeDomains":["example.com"]})
    );
    test::respond(
        "POST",
        "https://api.firecrawl.dev/v2/map",
        200,
        br#"{"success":true,"links":[]}"#,
    );
    calls::map(
        "https://example.com".into(),
        Some("docs".into()),
        Some(5),
        Some(true),
        Some(false),
    )
    .unwrap();
    let body: Value = serde_json::from_slice(&test::fetched()[1].body).unwrap();
    assert_eq!(
        body,
        json!({"url":"https://example.com","search":"docs","limit":5,"includeSubdomains":true,"ignoreQueryParameters":false})
    );
}

#[test]
fn omitted_map_limit_still_bounds_the_upstream_request() {
    test::respond(
        "POST",
        "https://api.firecrawl.dev/v2/map",
        200,
        br#"{"success":true,"links":[]}"#,
    );
    calls::map("https://example.com".into(), None, None, None, None).unwrap();
    let body: Value = serde_json::from_slice(&test::fetched()[0].body).unwrap();
    assert_eq!(body["limit"], 1000);
}

#[test]
fn crawl_submission_and_result_paging_remain_explicit() {
    test::respond(
        "POST",
        "https://api.firecrawl.dev/v2/crawl",
        200,
        br#"{"success":true,"id":"owned-job"}"#,
    );
    assert_eq!(
        calls::crawl(
            "https://example.com".into(),
            2,
            Some(1),
            Some(vec!["/docs/.*".into()]),
            None
        )
        .unwrap()["id"],
        "owned-job"
    );
    let body: Value = serde_json::from_slice(&test::fetched()[0].body).unwrap();
    assert_eq!(
        body,
        json!({"url":"https://example.com","limit":2,"maxDiscoveryDepth":1,"includePaths":["/docs/.*"],"scrapeOptions":{"parsers":[]}})
    );
    assert_eq!(test::fetched().len(), 1);
    test::respond("GET", "https://api.firecrawl.dev/v2/crawl/owned-job?skip=20", 200, br#"{"success":true,"status":"completed","creditsUsed":2,"data":[],"next":"https://api.firecrawl.dev/v2/crawl/owned-job?skip=40"}"#);
    let result = calls::crawl_status("owned-job".into(), Some(20)).unwrap();
    assert_eq!(result["creditsUsed"], 2);
    assert!(result["next"].is_string());
    assert_eq!(test::fetched().len(), 2);
}

#[test]
fn batch_options_and_failed_job_usage_are_preserved() {
    test::respond(
        "POST",
        "https://api.firecrawl.dev/v2/batch/scrape",
        200,
        br#"{"success":true,"id":"batch-job"}"#,
    );
    calls::batch_scrape(
        vec!["https://example.com".into()],
        vec![Format::Markdown, Format::Links],
        Some(false),
        Some(0),
    )
    .unwrap();
    let body: Value = serde_json::from_slice(&test::fetched()[0].body).unwrap();
    assert_eq!(
        body,
        json!({"urls":["https://example.com"],"formats":["markdown","links"],"onlyMainContent":false,"maxAge":0,"parsers":[]})
    );
    test::respond(
        "GET",
        "https://api.firecrawl.dev/v2/batch/scrape/batch-job",
        200,
        br#"{"success":true,"status":"failed","creditsUsed":1,"data":[]}"#,
    );
    let result = calls::batch_scrape_status("batch-job".into(), None).unwrap();
    assert_eq!(result["status"], "failed");
    assert_eq!(result["creditsUsed"], 1);
    assert_eq!(test::fetched().len(), 2);
}

#[test]
fn out_of_scope_sizes_and_offsets_fail_before_any_billable_request() {
    assert!(calls::search("q".into(), 11, None, None).is_err());
    assert!(calls::crawl("https://example.com".into(), 0, None, None, None).is_err());
    assert!(
        calls::batch_scrape(
            vec!["https://example.com".into(); 11],
            vec![Format::Html],
            None,
            None
        )
        .is_err()
    );
    assert!(calls::scrape("https://example.com".into(), Some(vec![]), None, None).is_err());
    assert!(calls::map("https://example.com".into(), None, Some(1001), None, None).is_err());
    assert!(calls::crawl_status("job".into(), Some(-1)).is_err());
    assert!(test::fetched().is_empty());
}

#[test]
fn application_failures_and_rate_limits_are_errors_without_retries() {
    let url = "https://api.firecrawl.dev/v2/scrape";
    test::respond(
        "POST",
        url,
        200,
        br#"{"success":false,"error":"No document","creditsUsed":0}"#,
    );
    assert!(
        calls::scrape("https://example.com".into(), None, None, None)
            .unwrap_err()
            .message
            .contains("No document")
    );
    test::respond_with(
        "POST",
        url,
        Response {
            status: 429,
            headers: vec![("Retry-After".into(), "30".into())],
            body: br#"{"error":"Rate limit"}"#.to_vec(),
        },
    );
    let error = calls::scrape("https://example.com".into(), None, None, None).unwrap_err();
    assert!(error.message.contains("retry-after=30"));
    assert_eq!(test::fetched().len(), 2);
}
