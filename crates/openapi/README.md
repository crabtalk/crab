# crab-openapi

Compile a local OpenAPI 3.0 or 3.1 JSON document into a berm service. The parser and generator run on the build host; generated requests use `berm_lang::http::Client`.

```rust
extern crate alloc;
use berm_lang::http::Client;

const CLIENT: Client<'static> = Client {
    name: "TikHub",
    headers: &[("Authorization", "Bearer {secret:TIKHUB_API_KEY}")],
};

crab_openapi::service! {
    spec: "openapi.json",
    client: crate::CLIENT,
    operations: {
        fetch_video: "GET /api/v1/douyin/web/fetch_one_video",
    },
}
```

Invoke once at the crate root. The consuming crate needs `berm-lang`, `url` and `extern crate alloc`. Define credentials, allowed HTTP hosts and tool metadata in its normal `crab.toml`.

- `api::fetch_video(&types::FetchVideoInput)` returns the complete upstream JSON response. Empty responses return JSON null; HTTP failures use the shared client's errors.
- Inputs group parameters under `path`, `query`, `headers` and `body`. Optional fields use `Option`; schema defaults remain upstream defaults.
- `tools::fetch_video(input)` exposes the API through `#[tool]`. Set `tools: false` for API-only bindings. Use `tools: cfg(feature = "tools")` to gate the tool module on the consuming crate's feature.
- `base_url: "https://example.com"` overrides the operation, path or document server. Otherwise the first applicable server is used.
- Spec files are compiler-tracked through `include_str!`; generation performs no network requests.

Supported requests use scalar path parameters occupying whole segments, form query parameters (scalars or scalar arrays), simple headers and JSON bodies. Local component references are resolved. External or recursive references, cookie parameters, server templates, other parameter encodings and deprecated operations produce compile errors.

Closed objects generate Rust structs; maps generate `BTreeMap`. Open objects with named properties, unions and nullable values retain `serde_json::Value`. Imported schema constraints remain in tool metadata; generated serde code checks Rust types, not every JSON Schema constraint. Provider-specific body-level error handling remains with the caller.

A build script can prepare the spec in `OUT_DIR`, then expose its path with `cargo::rustc-env=OPENAPI_SPEC=...`. Use `spec: env!("OPENAPI_SPEC")` in the invocation. Both the environment variable and the file are compiler-tracked. Download from a pinned source; downloaded specs belong in `OUT_DIR`, not the source tree.

OpenAPI 3.0 schemas are normalized before generation: nullable types, exclusive numeric bounds, examples and reference sibling semantics. Example payloads and Schema Object extensions are preserved.
