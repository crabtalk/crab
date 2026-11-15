use berm_api::digest;
use berm_build::source;
use std::{env, fs, path::PathBuf};

const SPECS: [(&str, &str, &str); 3] = [
    (
        "SEARCH_SPEC",
        "https://agent.tinyfish.ai/v1/openapi/search",
        "03d3bdb3cc85209374ca3f2e6a9a36fc01ba594c7136f79afcad72dfffc80a01",
    ),
    (
        "FETCH_SPEC",
        "https://agent.tinyfish.ai/v1/openapi/fetch",
        "bc3001a9f0adbf302eb53949785da7fac98f4b1aaf2b0e7c4e8e02fbf0243603",
    ),
    (
        "MAIN_SPEC",
        "https://agent.tinyfish.ai/v1/openapi/main",
        "62654eb2463213d46dc390855bc0d3fe5ab04c50f6b8bf488a37a610b422824d",
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
