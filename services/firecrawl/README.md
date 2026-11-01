# Firecrawl

Seven tools: `scrape`, `search`, `map`, `crawl`, `crawl_status`, `batch_scrape` and `batch_scrape_status`. They follow treg's bounded basic-operation scope. Agents, monitors, account endpoints and charged extraction add-ons are excluded.

Run `crab build`, then `crab deploy` here and supply `FIRECRAWL_API_KEY`. The first build needs `curl` and network access. `build.rs` downloads the official spec at revision `bde4de304dff4c0150854bdbfe605e188bab25f2` into `OUT_DIR` and adds the status controller's missing `skip` parameter to both status operations.

`calls` exposes the bounded operations; generated `api` and `types` expose the seven underlying requests. Disable default features to reuse either without tool exports.

Search, crawl and batch inputs are limited to ten results, pages or URLs. Map defaults to 1,000 links and accepts a smaller limit. Scrape and batch support Markdown, HTML and links; PDF parsing is disabled with `parsers: []`. Crawl fixes `scrapeOptions` to that same setting. Domain/path filters accept at most 20 entries. Optional fields are omitted when absent.

Crawl and batch submission return the upstream job ID. Call the corresponding status tool explicitly; pass `skip` from the returned `next` URL to read another page. Results retain status, page data and `creditsUsed`. Calls do not automatically poll, page or retry. The bounded calls report `success: false` as errors; generated raw API calls preserve the original JSON envelope.

[API reference](https://docs.firecrawl.dev/api-reference/v2-introduction) · [Status controller](https://github.com/firecrawl/firecrawl/blob/main/apps/api/src/controllers/v2/crawl-status.ts)
