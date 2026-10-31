# People Data Labs

Four generated tools cover treg's active PDL scope: person enrichment/search and company enrichment/search. Each has a catalog mapping in `crab.toml`. Autocomplete and the extended definitions are retired in treg and excluded; this does not describe their availability at PDL.

Run `crab build`, then `crab deploy` here and supply `PDL_API_KEY` from https://dashboard.peopledatalabs.com. The first build needs `curl` and network access. The official OpenAPI 3.0.3 document is downloaded at a pinned revision into `OUT_DIR`; parsing and generation run on the build host.

Inputs group GET parameters under `query`. Search accepts SQL or a JSON-encoded Elasticsearch query string, explicit page size and cursors. Responses retain upstream JSON, including match likelihood, matched fields, totals and cursors. HTTP 404 no-match responses remain errors; calls do not retry or paginate automatically. PDL bills search per returned record. Successful-response credit headers are not included in the JSON return value.

Disable default features to reuse `pdl::api` and `pdl::types` without tool exports. Schema constraints appear in tool metadata; Rust deserialization does not enforce every constraint.

[API documentation](https://docs.peopledatalabs.com/) · [Official specification](https://github.com/peopledatalabs/openAPI-specifications)
