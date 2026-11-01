use berm_lang::{
    http::Response,
    serde_json::{self, Value, json},
    test,
};
use litescrape::{api, news, types};

#[test]
fn news_selects_the_news_vertical_and_uses_the_deployment_key() {
    let expected = json!({"news_results":[{"title":"Example headline"}],"search_metadata":{"status":"Success"}});
    test::respond(
        "GET",
        "https://api.litescrape.com/api/google/search?q=AI+policy&hl=en&tbm=nws&num=5",
        200,
        &serde_json::to_vec(&expected).unwrap(),
    );
    assert_eq!(
        news::search_news("AI policy".into(), Some(5), Some("en".into())).unwrap(),
        expected
    );
    assert_eq!(
        test::fetched()[0].header("Authorization"),
        Some("Bearer {secret:LITESCRAPE_API_KEY}")
    );
}

#[test]
fn maps_preserves_explicit_pagination_without_fetching_another_page() {
    let input: types::GoogleMapsInput = serde_json::from_value(json!({
        "query": {"q":"coffee", "hl":"en", "start":20}
    }))
    .unwrap();
    test::respond("GET", "https://api.litescrape.com/api/google/maps?q=coffee&hl=en&start=20", 200, br#"{"local_results":[],"litescrape_pagination":{"next":"https://api.litescrape.com/api/google/maps?start=40"}}"#);
    let response = api::google_maps(&input).unwrap();
    assert!(response["litescrape_pagination"]["next"].is_string());
    assert_eq!(test::fetched().len(), 1);
}

#[test]
fn post_fetch_preserves_nested_browser_options_and_query_timeout() {
    let body = json!({
        "url":"https://example.com",
        "respond_with":"markdown",
        "wait_for_selector":["article","main"],
        "set_cookies":["theme=light; Path=/"],
        "viewport":{"width":1440,"height":900}
    });
    let input: types::WebFetchPostInput = serde_json::from_value(json!({
        "query":{"timeout":45.5}, "body":body
    }))
    .unwrap();
    test::respond(
        "POST",
        "https://api.litescrape.com/api/web/fetch?timeout=45.5",
        200,
        br##"{"url":"https://example.com","content":"# Example"}"##,
    );
    assert_eq!(api::web_fetch_post(&input).unwrap()["content"], "# Example");
    let request = &test::fetched()[0];
    assert_eq!(request.header("Content-Type"), Some("application/json"));
    assert_eq!(
        serde_json::from_slice::<Value>(&request.body).unwrap(),
        body
    );
}

#[test]
fn screenshots_return_base64_without_a_separate_download() {
    let input: types::WebScreenshotPostInput = serde_json::from_value(json!({
        "body":{"url":"https://example.com","full_page":true}
    }))
    .unwrap();
    test::respond(
        "POST",
        "https://api.litescrape.com/api/web/screenshot",
        200,
        br#"{"url":"https://example.com","screenshot_base64":"iVBORw0KGgo=","full_page":true}"#,
    );
    let response = api::web_screenshot_post(&input).unwrap();
    assert_eq!(response["screenshot_base64"], "iVBORw0KGgo=");
    assert_eq!(test::fetched().len(), 1);
}

#[test]
fn target_errors_and_concurrency_limits_preserve_upstream_details() {
    let input: types::WebFetchInput = serde_json::from_value(json!({
        "query":{"url":"https://example.com"}
    }))
    .unwrap();
    let url = "https://api.litescrape.com/api/web/fetch?url=https%3A%2F%2Fexample.com";
    test::respond(
        "GET",
        url,
        422,
        br#"{"error_code":"robots_txt_disallowed","error":"Blocked by robots.txt"}"#,
    );
    assert!(
        api::web_fetch(&input)
            .unwrap_err()
            .message
            .contains("robots_txt_disallowed")
    );
    test::respond_with(
        "GET",
        url,
        Response {
            status: 429,
            headers: vec![("Retry-After".into(), "15".into())],
            body: br#"{"error_code":"concurrency_busy"}"#.to_vec(),
        },
    );
    let error = api::web_fetch(&input).unwrap_err();
    assert!(error.message.contains("concurrency_busy"));
    assert!(error.message.contains("retry-after=15"));
    assert_eq!(test::fetched().len(), 2);
}
