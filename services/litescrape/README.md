# Litescrape

38 tools covering treg's Google, Bing and DuckDuckGo search, Maps, reviews, app stores, web fetch and screenshot operations. The complete generated operation list is in `src/lib.rs`; `news::search_news` fixes Google Search's vertical to `nws`. All tools have catalog mappings in `crab.toml`.

The obsolete screenshot-download route is excluded: the current upstream contract returns PNG bytes in `screenshot_base64`. Account, billing, async and additional scraper operations are outside this scope.

Run `crab build`, then `crab deploy` from this directory and supply `LITESCRAPE_API_KEY`. Builds need `curl` and network access on the first build. The downloaded OpenAPI 3.1 document stays in `OUT_DIR` and is checked against the SHA-256 pinned in `build.rs`. A changed upstream document requires reviewing and updating that pin; an existing matching cache remains usable.

Generated inputs group GET parameters under `query` and POST JSON under `body`; POST `timeout` remains a query parameter. POST fetch and screenshot tools accept nested browser options. Responses preserve upstream JSON, including pagination and screenshot data. Pass pagination parameters explicitly; calls do not automatically page or retry. Non-2xx responses retain the upstream error body and retry metadata.

Disable default features to reuse `litescrape::api`, `litescrape::types` and `litescrape::news` without tool exports.

[API reference](https://litescrape.com/docs/reference) · [OpenAPI contract](https://litescrape.com/api/openapi.json)
