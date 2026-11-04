use berm_lang::{
    serde_json::{self, Value, json},
    test,
};
use contextdev::{api, types};

#[test]
fn brand_supports_all_six_lookup_variants_with_host_managed_authentication() {
    for body in [
        json!({"type":"by_domain","domain":"example.com"}),
        json!({"type":"by_name","name":"Example"}),
        json!({"type":"by_email","email":"hi@example.com"}),
        json!({"type":"by_ticker","ticker":"EX","ticker_exchange":"NASDAQ"}),
        json!({"type":"by_direct_url","direct_url":"https://example.com/about"}),
        json!({"type":"by_transaction","transaction_info":"EXAMPLE ONLINE","country_gl":"us"}),
    ] {
        let input: types::BrandInput = serde_json::from_value(json!({"body":body})).unwrap();
        test::respond(
            "POST",
            "https://api.context.dev/v1/brand/retrieve",
            200,
            br#"{"brand":{"name":"Example"},"request_id":"req-1"}"#,
        );
        assert_eq!(api::brand(&input).unwrap()["request_id"], "req-1");
        let requests = test::fetched();
        let request = requests.last().unwrap();
        assert_eq!(
            request.header("Authorization"),
            Some("Bearer {secret:CONTEXT_DEV_API_KEY}")
        );
        assert_eq!(request.header("Content-Type"), Some("application/json"));
        assert_eq!(
            serde_json::from_slice::<Value>(&request.body).unwrap(),
            body
        );
    }
    assert_eq!(test::fetched().len(), 6);
}

#[test]
fn scrape_preserves_output_options_and_recursive_extraction_rules() {
    let body = json!({
        "url":"https://example.com", "formats":{"markdown":true,"screenshot":true,"product":true,"parse":true},
        "parseParams":{"rules":{"items":{"selector":"article","type":"list","output":{"title":"h1","links":{"selector":"a","type":"list","output":"@href"}}}}},
        "sharedParams":{"mainContentOnly":true}, "maxAgeMs":0,
        "timeoutOpts":{"milliseconds":10000,"behavior":"return-partial"}
    });
    let input: types::ScrapeInput = serde_json::from_value(json!({"body":body})).unwrap();
    let response = json!({"url":"https://example.com","partial":true,"markdown":"# Example","product":{"name":"Example"},"request_id":"req-2","key_metadata":{"credits_consumed":1}});
    test::respond(
        "POST",
        "https://api.context.dev/v1/web/scrape",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::scrape(&input).unwrap(), response);
    assert_eq!(
        serde_json::from_slice::<Value>(&test::fetched()[0].body).unwrap(),
        body
    );
    assert_eq!(test::fetched().len(), 1);
}

#[test]
fn styleguide_encodes_deadlines_and_explicit_cache_refresh() {
    let input: types::StyleguideInput = serde_json::from_value(json!({"query":{
        "directUrl":"https://example.com/about", "maxAgeMs":0,"colorScheme":"dark",
        "timeoutOpts":{"milliseconds":5000,"behavior":"return-partial"},"policies":["pol_one","pol_two"]
    }})).unwrap();
    test::respond(
        "GET",
        "https://api.context.dev/v1/web/styleguide?directUrl=https%3A%2F%2Fexample.com%2Fabout&maxAgeMs=0&timeoutOpts%5Bbehavior%5D=return-partial&timeoutOpts%5Bmilliseconds%5D=5000&colorScheme=dark&policies=pol_one,pol_two",
        200,
        br#"{"styleguide":{"typography":{}},"partial":true}"#,
    );
    assert_eq!(api::styleguide(&input).unwrap()["partial"], true);
}

#[test]
fn map_encodes_target_headers_filters_and_deadlines() {
    let input: types::MapInput = serde_json::from_value(json!({"query":{
        "domain":"example.com", "maxLinks":10,"includeSubdomains":true,
        "headers":{"X-Region":"eu west"},"timeoutOpts":{"milliseconds":5000},"tags":["production","team-alpha"]
    }})).unwrap();
    test::respond(
        "GET",
        "https://api.context.dev/v1/web/urls?domain=example.com&includeSubdomains=true&maxLinks=10&X-Region=eu+west&timeoutOpts%5Bmilliseconds%5D=5000&tags=production,team-alpha",
        200,
        br#"{"urls":[],"partial":true}"#,
    );
    assert_eq!(api::map(&input).unwrap()["partial"], true);
    assert_eq!(test::fetched().len(), 1);
}

#[test]
fn search_answers_and_crawl_preserve_their_json_requests() {
    let examples = [
        (
            "search",
            json!({"query":"release updates","numResults":5,"includeDomains":["example.com"],"markdownOptions":{"enabled":true},"freshness":"last_week"}),
        ),
        (
            "answers",
            json!({"task":"Find the current release","mode":"fast","json_format":{"version":"","sources":[]}}),
        ),
        (
            "crawl",
            json!({"url":"https://example.com","maxPages":5,"maxDepth":1,"urlRegex":"/docs/"}),
        ),
    ];
    for (name, body) in examples {
        let url = format!("https://api.context.dev/v1/web/{name}");
        test::respond("POST", &url, 200, br#"{"partial":true,"sources":[]}"#);
        let input = json!({"body":body});
        let result = match name {
            "search" => api::search(&serde_json::from_value(input).unwrap()),
            "answers" => api::answers(&serde_json::from_value(input).unwrap()),
            "crawl" => api::crawl(&serde_json::from_value(input).unwrap()),
            _ => unreachable!(),
        }
        .unwrap();
        assert_eq!(result["partial"], true);
        assert_eq!(
            serde_json::from_slice::<Value>(&test::fetched().last().unwrap().body).unwrap(),
            body
        );
    }
    assert_eq!(test::fetched().len(), 3);
}

#[test]
fn insufficient_credits_and_rate_limits_keep_upstream_error_details() {
    let input: types::BrandInput =
        serde_json::from_value(json!({"body":{"type":"by_domain","domain":"example.com"}}))
            .unwrap();
    test::respond(
        "POST",
        "https://api.context.dev/v1/brand/retrieve",
        402,
        br#"{"message":"Insufficient credits","error_code":"USAGE_EXCEEDED","credits_required":2}"#,
    );
    let error = api::brand(&input).unwrap_err().to_string();
    assert!(error.contains("402") && error.contains("USAGE_EXCEEDED"));
    test::respond(
        "POST",
        "https://api.context.dev/v1/brand/retrieve",
        429,
        br#"{"message":"Rate limited"}"#,
    );
    let error = api::brand(&input).unwrap_err().to_string();
    assert!(error.contains("429") && error.contains("Rate limited"));
    assert_eq!(test::fetched().len(), 2);
}
