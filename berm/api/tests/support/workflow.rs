use berm_api::Image;
use serde_json::{Value, json};

pub fn image(config: Value) -> Vec<u8> {
    let wasm = wat::parse_str(include_str!("../fixtures/workflow-only.wat")).unwrap();
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
    let image = Image(&wasm)
        .with_section(berm_abi::ABI_SECTION, &serde_json::to_vec(&record).unwrap())
        .unwrap();
    let service = json!({
        "name":"workflow-only","buffer":16384,
        "workflows":{"identity":config}
    });
    Image(&image)
        .with_section(
            berm_abi::SERVICE_SECTION,
            &serde_json::to_vec(&service).unwrap(),
        )
        .unwrap()
}
