use berm_api::digest;
use berm_build::source;
use std::{env, fs, path::PathBuf};

const SPECS: [(&str, &str, &str); 3] = [
    (
        "SEARCH_SPEC",
        "https://agent.tinyfish.ai/v1/openapi/search",
        "78a68188c2851d22f155afe4c409765cdc6c405d0152ac5a519f8ceeecf0d304",
    ),
    (
        "FETCH_SPEC",
        "https://agent.tinyfish.ai/v1/openapi/fetch",
        "7b56b3d1395f906e219973fb771c0c5faae92cb035ddb211f79e729b6b8b611b",
    ),
    (
        "MAIN_SPEC",
        "https://agent.tinyfish.ai/v1/openapi/main",
        "81be766c2fc28d8da7e1b692bc58b19aefe95cfd481b517808bcd7eb28fd8c7d",
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
