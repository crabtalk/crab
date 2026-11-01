#![cfg(feature = "tools")]
use berm_api::Manifest;
use berm_build::fixture;
use berm_lang::serde_json;

#[test]
fn image_contains_the_scoped_tools_and_nested_browser_constraints() {
    let bytes = fixture::image(env!("CARGO_PKG_NAME")).unwrap();
    let manifest = Manifest::try_from(bytes.as_slice()).unwrap();
    assert_eq!(manifest.tools.len(), 38);
    assert!(manifest.tools.iter().any(|tool| tool.name == "search_news"));
    assert!(
        !manifest
            .tools
            .iter()
            .any(|tool| tool.name.contains("download"))
    );
    let fetch = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "web_fetch_post")
        .unwrap();
    let properties = &fetch.parameters["properties"]["input"]["properties"];
    assert_eq!(properties["query"]["properties"]["timeout"]["maximum"], 90);
    assert_eq!(properties["body"]["required"], serde_json::json!(["url"]));
    assert!(properties["body"]["properties"].get("viewport").is_some());
}
