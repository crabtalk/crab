# Context.dev

Seven generated tools: brand lookup, styleguide extraction, scrape, answers, search, map and crawl. Brand lookup accepts domain, name, email, ticker, transaction or page URL. Scrape supports the current output formats and extraction options, including screenshots and products.

This covers the current equivalents of treg's `branddev` operations. Legacy simplified lookup has no separate current route; ISIN and NAICS lookup are excluded. Industry labels in current brand profiles use EIC. People, company and news search, batches, monitors and account administration are outside this scope.

Run `crab build`, then `crab deploy` here and supply `CONTEXT_DEV_API_KEY`. The first build needs `curl` and network access. The OpenAPI document is downloaded into `OUT_DIR` and verified against the SHA-256 in `build.rs`; upstream changes require reviewing and updating that pin.

Inputs group GET parameters under `query` and POST JSON under `body`. Responses retain upstream JSON, including sources, partial results and credit metadata. HTTP errors retain upstream details. Calls make one request without automatic polling, pagination or retries. Schema constraints appear in tool metadata; Rust deserialization does not enforce every constraint.

Disable default features to reuse `contextdev::api` and `contextdev::types` without tool exports. Search, scrape, map and crawl have catalog mappings; brand, styleguide and answers are callable directly.

[API reference](https://docs.context.dev) · [OpenAPI contract](https://docs.context.dev/openapi.json)
