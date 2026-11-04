#![cfg(feature = "tools")]
use berm_lang::test;
use dataforseo::tools;

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
