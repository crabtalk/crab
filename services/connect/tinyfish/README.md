# TinyFish

Search the web, news and academic publications, extract pages, or run an asynchronous browser agent using the deployment’s TinyFish account.

## Call

With Crab signed in to Cloud, replace `owner/tinyfish` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/tinyfish search '{"query":"Rust WebAssembly hosting"}'
```

Returns a page of search results. Use `search_news` or `search_publications` for those sources, `fetch` to extract URLs, and `run` for browser tasks.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crab.talk/docs/guides/mcp). Calls use this deployment’s provider credentials.

## Deploy your own

If this page offers **Deploy**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/tinyfish
```

Set `TINYFISH_API_KEY` through credential setup. Access and usage charges depend on the connected TinyFish account and wallet.

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source).

## Usage notes

`fetch` accepts 1–10 URLs and returns Markdown, HTML or JSON. Inspect both `results` and `errors` for partial failures. Conditional fetches accept one URL and may return null content when unchanged.

`run` returns a run ID. Poll `run_status` explicitly or call `run_cancel`. Enable screenshots or HTML in `options.capture_config` before calling `step_screenshot` or `step_html`. Screenshots return base64 JPEG; HTML returns text.

Failed or cancelled runs may still incur provider charges for completed steps. Calls make one upstream request, with explicit pagination and polling and no automatic retries.

Dedicated batch, monitor, wallet, profile, vault, synchronous automation, SSE, research-report, dataset and browser-session APIs are not included.

[Source and Rust API](https://github.com/crabtalk/crab/tree/main/services/connect/tinyfish) · [Provider documentation](https://docs.tinyfish.ai/)
