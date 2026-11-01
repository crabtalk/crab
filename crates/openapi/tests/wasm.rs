use berm_api::Manifest;
use berm_build::fixture;
use wasmparser::{Parser, Payload};

#[test]
fn generated_service_builds_with_tool_exports_and_imported_schemas() {
    let bytes = fixture::image("berm-openapi").unwrap();
    let manifest = Manifest::try_from(bytes.as_slice()).unwrap();
    assert_eq!(manifest.tools.len(), 4);
    let fetch = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "fetch_item")
        .unwrap();
    assert_eq!(
        fetch.parameters["properties"]["input"]["properties"]["query"]["properties"]["limit"]["maximum"],
        5
    );
    let exports: Vec<_> = Parser::new(0)
        .parse_all(&bytes)
        .filter_map(|payload| {
            if let Payload::ExportSection(exports) = payload.unwrap() {
                Some(
                    exports
                        .into_iter()
                        .map(|export| export.unwrap().name)
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        })
        .flatten()
        .collect();
    assert!(exports.contains(&"berm_tool_fetch_item"));
    assert!(exports.contains(&"berm_tool_create_item"));
}
