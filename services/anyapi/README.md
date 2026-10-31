# AnyAPI

184 operations matching treg's core and extended coverage, implemented from AnyAPI's official OpenAPI spec: Facebook, Google, Instagram, LinkedIn, Maps, Reddit, TikTok, TikTok Shop, X, YouTube and web extraction. The complete operation list is in `src/lib.rs`; 42 tools have catalog job mappings in `crab.toml`. The remaining 142 tools are callable without catalog jobs. Other run operations and discovery/account endpoints are excluded.

Run `crab build`, then `crab deploy` from this directory and supply `ANYAPI_API_KEY`. Builds need `curl` and network access on the first build. `build.rs` caches the unmodified spec from official SDK revision `674f737200a3962e5925963abcadb3947ba4941b` in `OUT_DIR`.

Inputs group the upstream JSON payload under `body`, optional response filters and `max_cost_usd` under `query`, and `Idempotency-Key` under `headers`. Pagination cursors and source preferences remain operation-specific. Pass a returned cursor explicitly for the next page. Calls are not automatically retried.

Responses preserve `output`, `costUsd`, `items` and provider metadata. `output.found: false` remains a successful result. HTTP failures preserve the upstream error body. Response filters do not reduce the upstream charge; use `max_cost_usd` to bound eligible source prices.

For source reuse, disable default features and call `anyapi::api` with types from `anyapi::types`.

[API documentation](https://getanyapi.com/docs/quickstart) · [Official SDK and spec](https://github.com/getanyapi-com/sdks)
