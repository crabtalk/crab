use serde_json::Value;
use std::{env, fs, path::PathBuf, process::Command};

const SPEC_URL: &str = "https://raw.githubusercontent.com/TikHub/TikHub-API-Python-SDK/1ede56bcc53f8c037744cbe2b70c93541b5cf840/spec/openapi.json";
const UNTYPED_LIMIT: &str = "/api/v1/bilibili/web/fetch_hot_search";
const FETCH_VIDEO: &str = "/api/v1/douyin/web/fetch_one_video";
const INVALID_EXAMPLES: &[&str] = &[
    "/paths/~1api~1v1~1youtube~1web_v2~1get_video_comments/get/parameters/3/schema/examples",
    "/paths/~1api~1v1~1youtube~1web_v2~1get_general_search/get/parameters/4/schema/examples",
    "/paths/~1api~1v1~1youtube~1web_v2~1get_general_search/get/parameters/5/schema/examples",
    "/paths/~1api~1v1~1youtube~1web_v2~1get_general_search/get/parameters/6/schema/examples",
    "/paths/~1api~1v1~1youtube~1web_v2~1get_general_search/get/parameters/7/schema/examples",
    "/paths/~1api~1v1~1youtube~1web_v2~1get_general_search/get/parameters/8/schema/examples",
    "/paths/~1api~1v1~1youtube~1web_v2~1get_shorts_search/get/parameters/4/schema/examples",
    "/paths/~1api~1v1~1youtube~1web_v2~1get_shorts_search/get/parameters/5/schema/examples",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let original = output.join("upstream.json");
    if !original.exists() {
        let download = output.join("upstream.download");
        let status = Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--retry",
                "2",
                "--max-time",
                "60",
                "--output",
            ])
            .arg(&download)
            .arg(SPEC_URL)
            .status()?;
        if !status.success() {
            return Err("TikHub spec download failed".into());
        }
        fs::rename(download, &original)?;
    }
    let mut spec: Value = serde_json::from_slice(&fs::read(original)?)?;
    for pointer in INVALID_EXAMPLES {
        let schema = spec
            .pointer_mut(pointer.strip_suffix("/examples").unwrap())
            .and_then(Value::as_object_mut)
            .ok_or_else(|| format!("missing TikHub schema: {pointer}"))?;
        let examples = schema
            .remove("examples")
            .ok_or_else(|| format!("missing TikHub examples: {pointer}"))?;
        assert!(examples.is_object(), "TikHub examples changed: {pointer}");
    }
    let paths = spec["paths"]
        .as_object_mut()
        .ok_or("missing TikHub paths")?;
    let operation = paths
        .get_mut(UNTYPED_LIMIT)
        .and_then(Value::as_object_mut)
        .and_then(|path| path.remove("get"));
    assert!(operation.is_some(), "missing excluded TikHub operation");

    let mut bindings = String::from(
        r#"crab_openapi::service! {
        spec: env!("OPENAPI_SPEC"),
        client: crate::CLIENT,
        base_url: "https://api.tikhub.io",
        operations: {
    "#,
    );
    let mut count = 0;
    for (path, item) in paths {
        for method in [
            "get", "post", "put", "patch", "delete", "head", "options", "trace",
        ] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            if operation["deprecated"] == true {
                continue;
            }
            let alias = if method == "get" && path == FETCH_VIDEO {
                "fetch_video".to_owned()
            } else {
                format!("operation_{count:04}")
            };
            let selector = format!("{} {path}", method.to_uppercase());
            bindings.push_str(&format!("{alias}: {selector:?},\n"));
            count += 1;
        }
    }
    assert_eq!(count, 1047, "TikHub operation coverage changed");
    bindings.push_str("}, }");
    fs::write(output.join("bindings.rs"), bindings)?;
    let prepared = output.join("openapi.json");
    fs::write(&prepared, serde_json::to_vec(&spec)?)?;
    println!("cargo::rustc-env=OPENAPI_SPEC={}", prepared.display());
    Ok(())
}
