#![cfg(feature = "tools")]
use berm_api::Manifest;
use std::{fs, path::Path};

#[test]
fn image_keeps_the_nine_tool_names_and_existing_input_shapes() {
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
            "fetch",
            "run",
            "run_cancel",
            "run_status",
            "search",
            "search_news",
            "search_publications",
            "step_html",
            "step_screenshot"
        ]
    );
    let search = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "search")
        .unwrap();
    assert!(search.parameters["properties"].get("query").is_some());
    assert!(search.parameters["properties"].get("options").is_some());
    let run = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "run")
        .unwrap();
    assert!(run.parameters["properties"].get("goal").is_some());
    assert!(run.parameters["properties"].get("options").is_some());
}
