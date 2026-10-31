use berm_build::source;
use serde_json::{Value, json};
use std::{env, fs, path::PathBuf};

const SPEC_URL: &str = "https://raw.githubusercontent.com/firecrawl/firecrawl-docs/bde4de304dff4c0150854bdbfe605e188bab25f2/api-reference/v2-openapi.json";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let original = output.join("upstream.json");
    source::download(SPEC_URL, &original)?;
    let mut spec: Value = serde_json::from_slice(&fs::read(original)?)?;
    // The v2 status controller accepts skip; this pinned spec omits it.
    for path in ["/crawl/{id}", "/batch/scrape/{id}"] {
        spec["paths"][path]["get"]["parameters"] = json!([{
            "name":"skip", "in":"query", "required":false,
            "description":"Offset from the next results URL.",
            "schema":{"type":"integer","minimum":0}
        }]);
    }
    let prepared = output.join("openapi.json");
    fs::write(&prepared, serde_json::to_vec(&spec)?)?;
    println!("cargo::rustc-env=OPENAPI_SPEC={}", prepared.display());
    Ok(())
}
