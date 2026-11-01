use berm_build::source;
use serde_json::Value;
use std::{env, fs, path::PathBuf};

const SPEC_URL: &str = "https://raw.githubusercontent.com/dataforseo/OpenApiDocumentation/55a4ed09d5295c96040a931449819d6e22418558/openapi_specification.yaml";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let upstream = out.join("upstream.yaml");
    source::download(SPEC_URL, &upstream)?;
    let spec: Value = serde_yaml_ng::from_slice(&fs::read(upstream)?)?;
    let spec_path = out.join("openapi.json");
    fs::write(&spec_path, serde_json::to_vec(&spec)?)?;
    println!("cargo::rustc-env=OPENAPI_SPEC={}", spec_path.display());
    Ok(())
}
