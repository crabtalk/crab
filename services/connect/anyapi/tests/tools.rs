#![cfg(feature = "tools")]
use anyapi::tools;
use berm_lang::test;

#[test]
fn generated_tools_reject_fields_outside_the_upstream_schema() {
    let error = test::call(
        tools::berm_tool_linkedin_company,
        br#"{"input":{"body":{"url":"https://www.linkedin.com/company/stripe","apiKey":"unexpected"}}}"#,
    )
    .unwrap_err();
    assert!(error.contains("unknown field"));
    assert!(test::fetched().is_empty());
}
