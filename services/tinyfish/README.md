# TinyFish

Nine tools matching [treg's TinyFish coverage](https://github.com/superdesigndev/treg/blob/main/src/treg/catalog/tinyfish.yaml), implemented against [TinyFish's API documentation](https://docs.tinyfish.ai/):

| Tool | Catalog job |
| --- | --- |
| `search` | `web.search` |
| `search_news` | `web.search.news` |
| `search_publications` | `web.search.publications` |
| `fetch` | `web.extract` |
| `run` | `web.agent.run` |
| `run_status` | `web.agent.run.status` |
| `run_cancel` | `web.agent.run.cancel` |
| `step_screenshot` | `web.agent.step.screenshot` |
| `step_html` | `web.agent.step.html` |

Search retains pagination, domains, freshness, language, location, thumbnails and attached fetch options. Fetch accepts 1–10 URLs as Markdown, HTML or JSON, with selectors, links, metadata and conditional fetching. Inspect both `results` and `errors` for partial failures. Conditional fetches accept one URL; unchanged content can be null.

`run` submits once and returns a run ID. Poll `run_status` for completion or call `run_cancel`. Enable screenshots or HTML in `capture_config` before retrieving those artifacts. Failed or cancelled runs can still incur upstream charges for completed steps. Screenshots return base64 JPEG; HTML returns text.

From `services/tinyfish`, run `crab build`, then `crab deploy`. Set `TINYFISH_API_KEY` through the approval page, granted to `agent.tinyfish.ai`, `api.search.tinyfish.ai` and `api.fetch.tinyfish.ai`. Credentials remain host-managed. Keep the deployment private if only your agents should use your provider account. Publish the image to a public registry to offer deployable copies. No credentials or fixed resale price are included in the seed.

Each call makes one upstream request. Pagination and polling are explicit; provider errors and retry timing are preserved without automatic retries. With `default-features = false`, the crate exposes the same operations as a Rust library without standalone tool exports.

Scope excludes dedicated batch, monitor, wallet, profile, vault, synchronous automation, SSE, research-report, dataset and browser-session APIs. This service has no generated SDK or API snapshots.
