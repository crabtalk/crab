use berm_api::digest;
use berm_build::source;
use std::{env, fs, path::PathBuf};

const SPECS: [(&str, &str, &str); 3] = [
    (
        "SEARCH_SPEC",
        "https://agent.tinyfish.ai/v1/openapi/search",
        "51f25b44b71b3183ad1a7b20635eed54d661af4f9de5e8592a87349a7ea84414",
    ),
    (
        "FETCH_SPEC",
        "https://agent.tinyfish.ai/v1/openapi/fetch",
        "e04a4d20c58e9817d787c86876ede61be2bb3d1a196e472de800c4d9f2c42707",
    ),
    (
        "MAIN_SPEC",
        "https://agent.tinyfish.ai/v1/openapi/main",
        "518f393238a6aab7e1b626e3bedb16c9866992c63eaa7c792812c227f7e9f10b",
    ),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for (variable, url, hash) in SPECS {
        let spec = out.join(format!("openapi-{hash}.json"));
        source::download(url, &spec)?;
        if digest::hex(&fs::read(&spec)?) != hash {
            return Err(format!(
                "TinyFish OpenAPI source changed: {url}; review and update the pinned digest"
            )
            .into());
        }
        println!("cargo::rustc-env={variable}={}", spec.display());
    }
    Ok(())
}
