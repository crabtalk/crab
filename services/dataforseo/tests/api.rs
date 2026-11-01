use berm_lang::{
    http::Response,
    serde_json::{self, Value, json},
    test,
};
use dataforseo::{api, types};

#[test]
fn keywords_keep_array_tasks_options_costs_and_basic_authentication() {
    let body = json!([{"keywords":["rust hosting","wasm hosting"],"location_code":2840,"language_code":"en","date_from":"2026-01-01"}]);
    let input: types::KeywordVolumeInput = serde_json::from_value(json!({"body":body})).unwrap();
    let response = json!({"status_code":20000,"cost":0.075,"tasks":[{"id":"kw-1","status_code":20000,"cost":0.075,"result":[{"keyword":"rust hosting","search_volume":100}]}]});
    test::respond(
        "POST",
        "https://api.dataforseo.com/v3/keywords_data/google_ads/search_volume/live",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::keyword_volume(&input).unwrap(), response);
    let request = &test::fetched()[0];
    assert_eq!(
        request.header("Authorization"),
        Some("Basic {secret:DATAFORSEO_AUTH_TOKEN}")
    );
    assert_eq!(request.header("Content-Type"), Some("application/json"));
    assert_eq!(
        serde_json::from_slice::<Value>(&request.body).unwrap(),
        body
    );
}

#[test]
fn backlink_filters_and_offsets_are_forwarded_without_automatic_pagination() {
    let body = json!([{"target":"example.com","limit":3,"offset":3,"filters":[["dofollow","=",true]],"order_by":["rank,desc"]}]);
    let input: types::BacklinksListInput = serde_json::from_value(json!({"body":body})).unwrap();
    let response = json!({"status_code":20000,"tasks":[{"status_code":20000,"cost":0.024,"result":[{"total_count":20,"items_count":3,"items":[]}]}]});
    test::respond(
        "POST",
        "https://api.dataforseo.com/v3/backlinks/backlinks/live",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::backlinks_list(&input).unwrap(), response);
    assert_eq!(
        serde_json::from_slice::<Value>(&test::fetched()[0].body).unwrap(),
        body
    );
    assert_eq!(test::fetched().len(), 1);
}

#[test]
fn task_creation_returns_the_id_without_polling() {
    let body = json!([{"keyword":"rust jobs","location_code":2840,"language_code":"en"}]);
    let input: types::SerpGoogleJobsTaskPostInput =
        serde_json::from_value(json!({"body":body})).unwrap();
    let response = json!({"status_code":20000,"tasks":[{"id":"job-1","status_code":20100,"cost":0.01,"result":null}]});
    test::respond(
        "POST",
        "https://api.dataforseo.com/v3/serp/google/jobs/task_post",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::serp_google_jobs_task_post(&input).unwrap(), response);
    assert_eq!(test::fetched().len(), 1);
}

#[test]
fn task_lookup_encodes_the_id_and_preserves_partial_results() {
    let input: types::OnPageSummaryIdInput =
        serde_json::from_value(json!({"path":{"id":"audit/1"}})).unwrap();
    let response = json!({"status_code":20000,"tasks":[{"id":"audit/1","status_code":40106,"status_message":"Task completed with partial results.","cost":0.02,"result":[{"pages_crawled":2}]}]});
    test::respond(
        "GET",
        "https://api.dataforseo.com/v3/on_page/summary/audit%2F1",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::on_page_summary_id(&input).unwrap(), response);
    assert_eq!(test::fetched().len(), 1);
}

#[test]
fn envelope_and_individual_task_failures_keep_ids_costs_and_successful_results() {
    let input: types::BacklinksSummaryInput =
        serde_json::from_value(json!({"body":[{"target":"example.com"}]})).unwrap();
    for response in [
        json!({"status_code":40200,"status_message":"Payment Required.","cost":0,"tasks":null}),
        json!({"status_code":20000,"cost":0.024,"tasks":[{"id":"ok-1","status_code":20000,"result":[{"backlinks":123}]},{"id":"bad-1","status_code":40501,"status_message":"Invalid Field.","cost":0}]}),
        json!({"status_code":20000,"tasks":[{"id":"blocked-1","status_code":40104,"status_message":"Please verify your account."}]}),
    ] {
        test::respond(
            "POST",
            "https://api.dataforseo.com/v3/backlinks/summary/live",
            200,
            &serde_json::to_vec(&response).unwrap(),
        );
        let error = api::backlinks_summary(&input).unwrap_err().to_string();
        assert!(error.contains(&serde_json::to_string(&response).unwrap()));
    }
    assert_eq!(test::fetched().len(), 3);
}

#[test]
fn http_failures_preserve_retry_metadata_and_do_not_retry() {
    let input = types::AccountUsageInput {};
    test::respond_with(
        "GET",
        "https://api.dataforseo.com/v3/appendix/user_data",
        Response {
            status: 429,
            headers: vec![
                ("Retry-After".into(), "10".into()),
                ("X-Request-Id".into(), "req-1".into()),
            ],
            body: br#"{"message":"rate limited"}"#.to_vec(),
        },
    );
    let error = api::account_usage(&input).unwrap_err().to_string();
    assert!(error.contains("429") && error.contains("rate limited"));
    assert!(error.contains("retry-after=10") && error.contains("x-request-id=req-1"));
    assert_eq!(test::fetched().len(), 1);
}
