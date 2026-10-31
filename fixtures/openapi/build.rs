use std::{env, fs, path::PathBuf};

fn main() {
    let source = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../crates/openapi/tests/specs/api.json");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("openapi.json");
    fs::copy(&source, &output).unwrap();
    println!("cargo::rerun-if-changed={}", source.display());
    println!("cargo::rustc-env=OPENAPI_SPEC={}", output.display());
}
