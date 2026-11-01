#![cfg(feature = "tools")]
use berm_api::Manifest;
use berm_lang::serde_json;
use std::{fs, path::Path};

#[test]
fn image_exports_only_the_seven_bounded_tools() {
    let image = berm_build::build(Path::new(env!("CARGO_MANIFEST_DIR")), None).unwrap();
    let bytes = fs::read(image).unwrap();
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
