use berm_api::digest;
use berm_build::source;
use std::{env, fs, path::PathBuf};

const SPEC_URL: &str = "https://docs.context.dev/openapi.json";
const SPEC_SHA256: &str = "80b47e67e2fa5eec03d917982f1d265d11c57446a214a3af5db57b293cf14617";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    let spec =
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join(format!("openapi-{SPEC_SHA256}.json"));
    source::download(SPEC_URL, &spec)?;
    if digest::hex(&fs::read(&spec)?) != SPEC_SHA256 {
        return Err(
            "Context.dev OpenAPI source changed; review and update the pinned digest".into(),
        );
    }
    println!("cargo::rustc-env=OPENAPI_SPEC={}", spec.display());
    Ok(())
}
