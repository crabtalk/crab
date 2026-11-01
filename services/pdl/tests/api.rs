use berm_lang::{
    http::Response,
    serde_json::{self, json},
    test,
};
use pdl::{api, types};

#[test]
fn person_enrichment_preserves_identifiers_matching_options_and_api_key() {
    let input: types::EnrichPersonInput = serde_json::from_value(json!({"query":{
        "email":"person@example.com", "profile":"https://www.linkedin.com/in/example-person",
        "min_likelihood":6, "required":"emails AND profiles", "include_if_matched":true,
        "data_include":"id,full_name", "titlecase":true
    }}))
    .unwrap();
    let response = json!({"status":200,"likelihood":8,"data":{"id":"example-1","full_name":"Example Person"},"matched":["email"]});
    test::respond(
        "GET",
        "https://api.peopledatalabs.com/v5/person/enrich?email=person%40example.com&profile=https%3A%2F%2Fwww.linkedin.com%2Fin%2Fexample-person&data_include=id%2Cfull_name&min_likelihood=6&include_if_matched=true&required=emails+AND+profiles&titlecase=true",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::enrich_person(&input).unwrap(), response);
    let request = &test::fetched()[0];
    assert_eq!(request.header("X-API-Key"), Some("{secret:PDL_API_KEY}"));
    assert!(request.body.is_empty());
    assert_eq!(request.header("Content-Type"), None);
}

#[test]
fn person_search_encodes_json_query_and_explicit_cursor_without_fetching_another_page() {
    let query = json!({"query":{"term":{"job_company_website":"example.com"}}}).to_string();
    let input: types::SearchPeopleInput = serde_json::from_value(json!({"query":{
        "query":query, "size":2,"scroll_token":"cursor+next/1","dataset":"resume","data_include":"id,job_title"
    }})).unwrap();
    let response =
        json!({"status":200,"data":[{"id":"person-1"}],"total":20,"scroll_token":"cursor-next-2"});
    test::respond(
        "GET",
        "https://api.peopledatalabs.com/v5/person/search?query=%7B%22query%22%3A%7B%22term%22%3A%7B%22job_company_website%22%3A%22example.com%22%7D%7D%7D&size=2&scroll_token=cursor%2Bnext%2F1&dataset=resume&data_include=id%2Cjob_title",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::search_people(&input).unwrap(), response);
    assert_eq!(test::fetched().len(), 1);
}

#[test]
fn company_enrichment_preserves_profile_ticker_location_and_field_requirements() {
    let input: types::EnrichCompanyInput = serde_json::from_value(json!({"query":{
        "website":"example.com","profile":"linkedin.com/company/example","ticker":"EX",
        "country":"united states","min_likelihood":6,"required":"website AND industry","include_if_matched":true
    }})).unwrap();
    let response = json!({"status":200,"likelihood":9,"id":"company-1","name":"Example","website":"example.com"});
    test::respond(
        "GET",
        "https://api.peopledatalabs.com/v5/company/enrich?profile=linkedin.com%2Fcompany%2Fexample&ticker=EX&website=example.com&country=united+states&include_if_matched=true&min_likelihood=6&required=website+AND+industry",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::enrich_company(&input).unwrap(), response);
}

#[test]
fn company_search_encodes_sql_and_keeps_empty_results() {
    let input: types::SearchCompaniesInput = serde_json::from_value(json!({"query":{
        "sql":"SELECT * FROM company WHERE website='example.com'", "size":1,"from":10,"scroll_token":"cursor-1"
    }})).unwrap();
    let response = json!({"status":200,"data":[],"total":0});
    test::respond(
        "GET",
        "https://api.peopledatalabs.com/v5/company/search?sql=SELECT+*+FROM+company+WHERE+website%3D%27example.com%27&size=1&from=10&scroll_token=cursor-1",
        200,
        &serde_json::to_vec(&response).unwrap(),
    );
    assert_eq!(api::search_companies(&input).unwrap(), response);
    assert_eq!(test::fetched().len(), 1);
}

#[test]
fn no_match_and_rate_limits_preserve_errors_without_retries() {
    let input: types::EnrichPersonInput =
        serde_json::from_value(json!({"query":{"email":"missing@example.com"}})).unwrap();
    let url = "https://api.peopledatalabs.com/v5/person/enrich?email=missing%40example.com";
    test::respond(
        "GET",
        url,
        404,
        br#"{"status":404,"error":{"type":"not_found","message":"No records found"}}"#,
    );
    let error = api::enrich_person(&input).unwrap_err().to_string();
    assert!(error.contains("404") && error.contains("No records found"));
    test::respond_with(
        "GET",
        url,
        Response {
            status: 429,
            headers: vec![("Retry-After".into(), "60".into())],
            body: br#"{"status":429,"error":{"message":"Rate limit exceeded"}}"#.to_vec(),
        },
    );
    let error = api::enrich_person(&input).unwrap_err().to_string();
    assert!(error.contains("429") && error.contains("retry-after=60"));
    assert_eq!(test::fetched().len(), 2);
}
