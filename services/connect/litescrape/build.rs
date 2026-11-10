use berm_api::digest;
use berm_build::source;
use std::{env, fs, path::PathBuf};

const SPEC_URL: &str = "https://litescrape.com/api/openapi.json";
const SPEC_SHA256: &str = "70acb5a2fb52315f7f6c8ac1a127a1559b949022dd3c6ed987bf875bab55c23a";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    let spec =
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join(format!("openapi-{SPEC_SHA256}.json"));
    source::download(SPEC_URL, &spec)?;
    if digest::hex(&fs::read(&spec)?) != SPEC_SHA256 {
        return Err(
            "Litescrape OpenAPI source changed; review and update the pinned digest".into(),
        );
    }
    println!("cargo::rustc-env=OPENAPI_SPEC={}", spec.display());
    Ok(())
}
