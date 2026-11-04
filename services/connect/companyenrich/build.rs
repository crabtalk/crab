use berm_api::digest;
use berm_build::source;
use serde_json::{Value, json};
use std::{env, error::Error, fs, path::PathBuf};

const DOCS_URL: &str = "https://docs.companyenrich.com/reference";
const JSON_FENCE: &str = "```json";

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=sources.sha256");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut merged = json!({
        "openapi": "3.0.1",
        "info": {"title": "CompanyEnrich", "version": "1.0.0"},
        "servers": [{"url": "https://api.companyenrich.com"}],
        "paths": {}, "components": {}
    });
    for line in include_str!("sources.sha256").lines() {
        let (expected, page) = line.split_once("  ").ok_or("invalid source digest entry")?;
        let destination = out.join(format!("{expected}-{page}"));
        source::download(&format!("{DOCS_URL}/{page}"), &destination)?;
        let bytes = fs::read(destination)?;
        if digest::hex(&bytes) != expected {
            return Err(
                format!("CompanyEnrich source changed: {page}; review its pinned digest").into(),
            );
        }
        let text = std::str::from_utf8(&bytes)?;
        let (_, definition) = text
            .split_once(JSON_FENCE)
            .ok_or("missing OpenAPI definition")?;
        let document = serde_json::Deserializer::from_str(definition)
            .into_iter::<Value>()
            .next()
            .ok_or("empty OpenAPI definition")??;
        if document["openapi"] != merged["openapi"] || document["servers"] != merged["servers"] {
            return Err(format!("inconsistent OpenAPI version or server: {page}").into());
        }
        for section in ["paths", "components"] {
            for (group, entries) in document[section]
                .as_object()
                .ok_or("missing OpenAPI section")?
            {
                let target = merged[section]
                    .as_object_mut()
                    .unwrap()
                    .entry(group)
                    .or_insert_with(|| json!({}))
                    .as_object_mut()
                    .unwrap();
                for (name, value) in entries.as_object().ok_or("invalid OpenAPI section")? {
                    if let Some(existing) = target.get(name) {
                        if existing != value {
                            return Err(
                                format!("conflicting {section}/{group}/{name}: {page}").into()
                            );
                        }
                    } else {
                        target.insert(name.clone(), value.clone());
                    }
                }
            }
        }
    }
    let spec = out.join("openapi.json");
    fs::write(&spec, serde_json::to_vec(&merged)?)?;
    println!("cargo::rustc-env=OPENAPI_SPEC={}", spec.display());
    Ok(())
}
