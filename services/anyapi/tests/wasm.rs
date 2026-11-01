#![cfg(feature = "tools")]
use berm_api::Manifest;
use std::{fs, path::Path};

#[test]
fn image_contains_the_complete_service_scope_and_imported_constraints() {
    let image = berm_build::build(Path::new(env!("CARGO_MANIFEST_DIR")), None).unwrap();
    let bytes = fs::read(image).unwrap();
    let manifest = Manifest::try_from(bytes.as_slice()).unwrap();
    assert_eq!(manifest.tools.len(), 184);
    let company = manifest
        .tools
        .iter()
        .find(|tool| tool.name == "linkedin_company")
        .unwrap();
    let schema = &company.parameters["properties"]["input"]["properties"];
    assert_eq!(
        schema["body"]["required"],
        berm_lang::serde_json::json!(["url"])
    );
    assert_eq!(schema["body"]["additionalProperties"], false);
    assert_eq!(
        schema["headers"]["properties"]["Idempotency-Key"]["maxLength"],
        255
    );
    assert!(schema["query"]["properties"].get("max_cost_usd").is_some());
}
