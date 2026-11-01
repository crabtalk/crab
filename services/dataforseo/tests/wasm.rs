#![cfg(feature = "tools")]
use berm_api::Manifest;
use berm_build::fixture;
use berm_lang::{serde_json, test};
use dataforseo::tools;

#[test]
fn image_exports_the_verified_scope_and_array_task_schemas() {
    let bytes = fixture::image(env!("CARGO_PKG_NAME")).unwrap();
    let manifest = Manifest::try_from(bytes.as_slice()).unwrap();
    assert_eq!(manifest.tools.len(), 240);
    assert!(
        manifest
            .tools
            .iter()
            .any(|tool| tool.name == "account_usage")
    );
    assert!(
        manifest
            .tools
            .iter()
            .any(|tool| tool.name == "serp_google_jobs_task_post")
    );
    assert!(
        manifest
            .tools
            .iter()
            .any(|tool| tool.name == "app_data_apple_app_info_task_get_advanced_id")
    );
    assert!(
        !manifest
            .tools
            .iter()
            .any(|tool| tool.name.contains("events_live") || tool.name.contains("pinterest"))
    );
    let keyword = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "keyword_volume")
        .unwrap();
    let body = &keyword.parameters["properties"]["input"]["properties"]["body"];
    assert_eq!(body["type"], "array");
    assert!(serde_json::to_string(body).unwrap().contains("keywords"));
}

#[test]
fn tool_exports_use_the_same_task_status_checks() {
    test::respond("POST", "https://api.dataforseo.com/v3/backlinks/summary/live", 200, br#"{"status_code":20000,"tasks":[{"status_code":40501,"status_message":"Invalid target."}]}"#);
    let error = test::call(
        tools::berm_tool_backlinks_summary,
        br#"{"input":{"body":[{"target":"example.com"}]}}"#,
    )
    .unwrap_err();
    assert!(error.to_string().contains("40501"));
}
