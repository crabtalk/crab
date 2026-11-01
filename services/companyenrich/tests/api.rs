use berm_lang::{
    http::Response,
    serde_json::{self, Value, json},
    test,
};
use companyenrich::{api, types};

#[test]
fn company_lookup_encodes_expansion_and_uses_host_managed_bearer_authentication() {
    let input: types::EnrichCompanyInput = serde_json::from_value(json!({"query":{
        "domain":"example.com", "expand":["workforce"], "waitForEnrichment":false
    }}))
    .unwrap();
    let response = json!({"id":"company-1","domain":"example.com","workforce":{"total":20}});
    test::respond(
        "GET",
        "https://api.companyenrich.com/companies/enrich?domain=example.com&expand=workforce&waitForEnrichment=false",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::enrich_company(&input).unwrap(), response);
    let request = &test::fetched()[0];
    assert_eq!(
        request.header("Authorization"),
        Some("Bearer {secret:COMPANYENRICH_API_KEY}")
    );
    assert!(request.body.is_empty());
}

#[test]
fn search_keeps_nested_filters_nulls_and_explicit_pagination() {
    let body = json!({"page":2,"pageSize":5,"semanticQuery":"cloud infrastructure",
        "countries":["US"],"technologies":["Rust"],"exclude":{"domains":["excluded.example"]},
        "workforceGrowth":{"period":"m6","minPercent":10},"query":null});
    let input: types::SearchCompaniesInput =
        serde_json::from_value(json!({"body":body,"query":{"expand":["workforce"]}})).unwrap();
    let response = json!({"items":[{"domain":"example.com"}],"totalItems":40,"page":2});
    test::respond(
        "POST",
        "https://api.companyenrich.com/companies/search?expand=workforce",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::search_companies(&input).unwrap(), response);
    assert_eq!(test::fetched().len(), 1);
    assert_eq!(
        serde_json::from_slice::<Value>(&test::fetched()[0].body).unwrap(),
        body
    );
}

#[test]
fn people_scroll_keeps_education_filters_and_cursor_without_followup_requests() {
    let body = json!({"pageSize":3,"cursor":"next+1/2","domains":["example.com"],
        "positionQuery":["engineer"],"seniority":["senior"],"education":{"institutionNameQuery":["Example University"]}});
    let input: types::ScrollPeopleInput =
        serde_json::from_value(json!({"body":body,"query":{"expand":["education"]}})).unwrap();
    let response = json!({"items":[{"id":123,"firstName":"Example"}],"nextCursor":"next-2"});
    test::respond(
        "POST",
        "https://api.companyenrich.com/people/search/scroll?expand=education",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::scroll_people(&input).unwrap(), response);
    assert_eq!(test::fetched().len(), 1);
    assert_eq!(
        serde_json::from_slice::<Value>(&test::fetched()[0].body).unwrap(),
        body
    );
}

#[test]
fn bulk_email_jobs_keep_outcome_counts_and_require_explicit_status_calls() {
    let body = json!({"items":[{"person_id":123,"domain":"example.com"}],"webhook_url":"https://example.com/hook"});
    let input: types::StartBulkEmailEnrichmentInput =
        serde_json::from_value(json!({"body":body})).unwrap();
    test::respond(
        "POST",
        "https://api.companyenrich.com/people/email/bulk",
        200,
        br#"{"job_id":"job-1","status":"pending"}"#,
    );
    assert_eq!(
        api::start_bulk_email_enrichment(&input).unwrap()["job_id"],
        "job-1"
    );
    assert_eq!(test::fetched().len(), 1);
    assert_eq!(
        serde_json::from_slice::<Value>(&test::fetched()[0].body).unwrap(),
        body
    );
    let input: types::GetBulkEmailEnrichmentInput =
        serde_json::from_value(json!({"path":{"jobId":"job-1"}})).unwrap();
    let response = json!({"job_id":"job-1","status":"completed","total_requests":1,"found_count":0,"not_found_count":1,"credits_charged":0,"results_url":"https://example.com/emails.json"});
    test::respond(
        "GET",
        "https://api.companyenrich.com/people/email/bulk/job-1",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::get_bulk_email_enrichment(&input).unwrap(), response);
    assert_eq!(test::fetched().len(), 2);
}

#[test]
fn company_export_preserves_search_and_webhook_without_downloading_results() {
    let body = json!({"count":10,"search":{"countries":["US"]},"webhook_url":"https://example.com/export"});
    let input: types::StartCompanyExportInput =
        serde_json::from_value(json!({"body":body})).unwrap();
    test::respond(
        "POST",
        "https://api.companyenrich.com/companies/search/async",
        200,
        br#"{"job_id":"export-1","status":"pending"}"#,
    );
    assert_eq!(
        api::start_company_export(&input).unwrap()["job_id"],
        "export-1"
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&test::fetched()[0].body).unwrap(),
        body
    );
    let input: types::GetCompanyExportInput =
        serde_json::from_value(json!({"path":{"jobId":"export-1"}})).unwrap();
    let response = json!({"job_id":"export-1","status":"completed","results_url":"https://example.com/results.json"});
    test::respond(
        "GET",
        "https://api.companyenrich.com/companies/search/async/export-1",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::get_company_export(&input).unwrap(), response);
    assert_eq!(test::fetched().len(), 2);
}

#[test]
fn job_filters_support_nullable_enum_schema_and_path_segments_are_encoded() {
    let input: types::ListJobsInput = serde_json::from_value(
        json!({"query":{"page":2,"pageSize":5,"status":"completed","type":"bulk"}}),
    )
    .unwrap();
    test::respond(
        "GET",
        "https://api.companyenrich.com/jobs?page=2&pageSize=5&status=completed&type=bulk",
        200,
        br#"{"items":[],"totalItems":0}"#,
    );
    assert_eq!(api::list_jobs(&input).unwrap()["totalItems"], 0);
    let input: types::GetJobInput =
        serde_json::from_value(json!({"path":{"jobId":"a/b+c"}})).unwrap();
    test::respond(
        "GET",
        "https://api.companyenrich.com/jobs/a%2Fb+c",
        200,
        br#"{"status":"processing"}"#,
    );
    assert_eq!(api::get_job(&input).unwrap()["status"], "processing");
}

#[test]
fn no_match_and_rate_limit_errors_retain_problem_details_and_retry_metadata() {
    let input: types::FindPersonEmailInput =
        serde_json::from_value(json!({"query":{"id":123,"domain":"example.com"}})).unwrap();
    let url = "https://api.companyenrich.com/people/email?id=123&domain=example.com";
    test::respond(
        "GET",
        url,
        404,
        br#"{"title":"Not Found","detail":"Email could not be found"}"#,
    );
    let error = api::find_person_email(&input).unwrap_err().to_string();
    assert!(error.contains("404") && error.contains("Email could not be found"));
    test::respond_with(
        "GET",
        url,
        Response {
            status: 429,
            headers: vec![("Retry-After".into(), "60".into())],
            body: br#"{"title":"Too Many Requests"}"#.to_vec(),
        },
    );
    let error = api::find_person_email(&input).unwrap_err().to_string();
    assert!(error.contains("429") && error.contains("retry-after=60"));
    assert_eq!(test::fetched().len(), 2);
}
