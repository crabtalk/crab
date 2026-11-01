# DataForSEO

240 generated tools cover the matching treg core and extended operations: backlinks, keyword metrics, rankings, SERPs, audits, business and app data, AI visibility, and the listed task submissions and retrievals. The operation list is in `src/lib.rs`; 20 tools have catalog mappings in `crab.toml`.

Google Events Live Advanced and Pinterest Live are absent from the pinned specification and excluded. No additional vendor operations are exported.

Run `crab build`, then `crab deploy` here. Supply `DATAFORSEO_AUTH_TOKEN`: Base64-encoded `API_LOGIN:API_PASSWORD`, without the `Basic ` prefix. API credentials are available at https://app.dataforseo.com/api-access; the API password differs from your account password. The host substitutes the encoded credential.

The first build needs `curl` and network access. `build.rs` downloads the official OpenAPI 3.0.1 YAML at a pinned revision and converts it to JSON in `OUT_DIR`. Parsing and generation run on the build host.

POST inputs use `body`, an array of task objects; GET path parameters use `path`. Live endpoints accept one task per call; Standard task submission limits vary by endpoint. Responses retain task IDs, results and costs. Envelope and task failures return errors containing the original response, including successful tasks in a mixed result. Created tasks and explicitly partial results remain responses. Calls do not automatically poll, paginate or retry.

Disable default features to reuse `dataforseo::api` and `dataforseo::types` without tool exports. Schema constraints appear in tool metadata; Rust deserialization does not enforce every constraint.

[API documentation](https://docs.dataforseo.com/v3/) · [Official specification](https://github.com/dataforseo/OpenApiDocumentation)
