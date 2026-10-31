use berm_build::source;
use std::{env, path::PathBuf};

const SPEC_URL: &str = "https://raw.githubusercontent.com/peopledatalabs/openAPI-specifications/65d329510d6d410e79748cbf08ff919df05284b0/pdl-specs.json";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    let spec = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("openapi.json");
    source::download(SPEC_URL, &spec)?;
    println!("cargo::rustc-env=OPENAPI_SPEC={}", spec.display());
    Ok(())
}
