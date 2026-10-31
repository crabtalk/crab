# TikHub generation fixture

`build.rs` downloads the pinned upstream spec into `OUT_DIR`, removes eight malformed schema `examples` annotations, and excludes `GET /api/v1/bilibili/web/fetch_hot_search` because its `limit` parameter has no declared type. The remaining 1,047 operations generate API bindings and tools. Numeric operation aliases are fixture identifiers.

Run `cargo nextest run -p berm-tikhub`. The first build requires network access and `curl`; requests in tests are mocked.
