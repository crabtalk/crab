use berm_api::digest;
use berm_build::source;
use std::{env, fs, path::PathBuf};

const SPEC_URL: &str = "https://docs.companyenrich.com/openapi/v1.json";
const SPEC_SHA256: &str = "51432753f0921bfbfd903979b87c40b15787ad17e5bb31cd68bc9032f483be1d";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    let spec =
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join(format!("openapi-{SPEC_SHA256}.json"));
    source::download(SPEC_URL, &spec)?;
    if digest::hex(&fs::read(&spec)?) != SPEC_SHA256 {
        return Err(
            "CompanyEnrich OpenAPI source changed; review and update the pinned digest".into(),
        );
    }
    println!("cargo::rustc-env=OPENAPI_SPEC={}", spec.display());
    Ok(())
}
