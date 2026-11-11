# Litescrape

Search Google, Bing and DuckDuckGo, read maps and reviews, look up app-store data, fetch web content and capture screenshots.

## Call

With Crab signed in to Cloud, replace `owner/litescrape` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/litescrape google_maps '{"input":{"query":{"q":"coffee","hl":"en","start":0}}}'
```

Returns upstream local results and available pagination metadata. Use `search_news` for Google News, or the fetch and screenshot tools for page content.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crabtalk.ai/docs/guides/mcp). Calls use this deployment’s provider credentials.

## Deploy your own

If this page offers **Deploy**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/litescrape
```

Set `LITESCRAPE_API_KEY` through the copy’s credential setup. Calls consume the connected account’s prepaid credits and concurrency allowance.

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source).

## Usage notes

GET parameters go under `input.query` and POST JSON under `input.body`; POST `timeout` remains a query parameter. POST fetch and screenshot tools accept nested browser options.

Pass pagination parameters explicitly. Responses retain provider metadata and screenshot data; screenshots are PNG bytes encoded in `screenshot_base64`.

Non-2xx errors retain the provider body and retry metadata. Calls do not automatically paginate or retry. Account, billing, async and additional scraper operations are not included.

[Source and Rust API](https://github.com/crabtalk/crab/tree/main/services/connect/litescrape) · [Provider documentation](https://litescrape.com/docs/reference)
