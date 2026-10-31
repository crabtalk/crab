#![cfg(feature = "tools")]
use berm_api::Manifest;
use std::{fs, path::Path};

#[test]
fn image_exports_active_tools_and_full_query_constraints() {
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
            "enrich_company",
            "enrich_person",
            "search_companies",
            "search_people"
        ]
    );
    let search = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "search_people")
        .unwrap();
    let properties = &search.parameters["properties"]["input"]["properties"]["query"]["properties"];
    assert_eq!(properties["size"]["minimum"], 1);
    assert_eq!(properties["size"]["maximum"], 100);
    assert!(properties.get("scroll_token").is_some());
    let enrich = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "enrich_person")
        .unwrap();
    let properties = &enrich.parameters["properties"]["input"]["properties"]["query"]["properties"];
    assert_eq!(properties["min_likelihood"]["maximum"], 10);
    assert!(properties.get("email_hash").is_some());
    assert!(properties.get("data_include").is_some());
}
