#![cfg(feature = "tools")]
use berm_api::Manifest;
use berm_build::fixture;
use berm_lang::serde_json;

#[test]
fn image_exports_only_the_seven_bounded_tools() {
    let bytes = fixture::image(env!("CARGO_PKG_NAME")).unwrap();
    let manifest = Manifest::try_from(bytes.as_slice()).unwrap();
    assert_eq!(manifest.tools.len(), 7);
    let scrape = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "scrape")
        .unwrap();
    assert_eq!(
        scrape.parameters["properties"]["formats"]["items"]["enum"],
        serde_json::json!(["markdown", "html", "links"])
    );
    assert!(scrape.parameters["properties"].get("parsers").is_none());
}
