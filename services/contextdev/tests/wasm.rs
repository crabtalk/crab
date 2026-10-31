#![cfg(feature = "tools")]
use berm_api::Manifest;
use berm_lang::serde_json;
use std::{fs, path::Path};

#[test]
fn image_exports_seven_tools_with_lookup_variants_and_recursive_parse_rules() {
    let image = berm_build::build(Path::new(env!("CARGO_MANIFEST_DIR")), None).unwrap();
    let manifest = Manifest::try_from(fs::read(image).unwrap().as_slice()).unwrap();
    let mut names: Vec<_> = manifest
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "answers",
            "brand",
            "crawl",
            "map",
            "scrape",
            "search",
            "styleguide"
        ]
    );
    let brand = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "brand")
        .unwrap();
    let variants = &brand.parameters["properties"]["input"]["properties"]["body"]["oneOf"];
    assert_eq!(variants.as_array().unwrap().len(), 6);
    let scrape = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "scrape")
        .unwrap();
    let properties = &scrape.parameters["properties"]["input"]["properties"]["body"]["properties"];
    assert_eq!(
        properties["formats"]["properties"]["screenshot"]["type"],
        "boolean"
    );
    assert!(properties.get("parseParams").is_some());
    assert!(
        serde_json::to_string(&scrape.parameters)
            .unwrap()
            .contains("urn:crab-openapi:schema:")
    );
}
