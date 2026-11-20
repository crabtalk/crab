#![allow(dead_code)]

use berm_api::Image;
use serde_json::{Value, json};

pub fn image(config: Value) -> Vec<u8> {
    let wasm = wat::parse_str(include_str!("../fixtures/workflow-only.wat")).unwrap();
    with_wasm(&wasm, config)
}

pub fn with_wasm(wasm: &[u8], config: Value) -> Vec<u8> {
    let record = json!({
        "workflow": {
            "version": 1, "name": "identity",
            "graph": {
                "input": {"type":"integer","minimum":0},
                "output": {"type":"integer","minimum":0},
                "nodes": [{
                    "id":"done","export":"berm_workflow_identity_done",
                    "dependencies":[{"source":{"kind":"input"},"optional":false,"case":null}],
                    "guards":[], "kind":"output"
                }]
            }
        },
        "abi_version":berm_abi::ABI_VERSION, "crate":"workflow-only"
    });
    with_record(wasm, record, config)
}

pub fn with_record(wasm: &[u8], mut record: Value, mut config: Value) -> Vec<u8> {
    let schedule = config
        .as_object_mut()
        .and_then(|config| config.remove("schedule"));
    record["config"] = config;
    let image = Image(wasm)
        .with_section(berm_abi::ABI_SECTION, &serde_json::to_vec(&record).unwrap())
        .unwrap();
    let service = json!({
        "name":"workflow-only","buffer":16384,
        "workflows": schedule.map(|schedule| json!({"identity":{"schedule":schedule}})).unwrap_or_else(|| json!({}))
    });
    Image(&image)
        .with_section(
            berm_abi::SERVICE_SECTION,
            &serde_json::to_vec(&service).unwrap(),
        )
        .unwrap()
}
