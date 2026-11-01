use berm_build::source;
use std::{env, path::PathBuf};

const SPEC_URL: &str = "https://raw.githubusercontent.com/getanyapi-com/sdks/674f737200a3962e5925963abcadb3947ba4941b/openapi.json";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    let spec = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("openapi.json");
    source::download(SPEC_URL, &spec)?;
    println!("cargo::rustc-env=OPENAPI_SPEC={}", spec.display());
    Ok(())
}
