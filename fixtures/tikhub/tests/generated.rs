use berm_lang::{
    serde_json::{self, Value, json},
    test,
};
use berm_tikhub::tools;

#[test]
fn real_douyin_schema_generates_the_expected_http_request() {
    test::respond(
        "GET",
        "https://api.tikhub.io/api/v1/douyin/web/fetch_one_video?aweme_id=7251234567890123456&need_anchor_info=true",
        200,
        br#"{"code":200,"data":{"aweme_id":"7251234567890123456"}}"#,
    );
    let response = test::call(
        tools::berm_tool_fetch_video,
        br#"{"input":{"query":{"aweme_id":"7251234567890123456","need_anchor_info":true}}}"#,
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&response).unwrap(),
        json!({"code":200,"data":{"aweme_id":"7251234567890123456"}})
    );
    assert_eq!(
        test::fetched()[0].header("Authorization"),
        Some("Bearer {secret:TIKHUB_API_KEY}")
    );
}
