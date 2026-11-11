# Firecrawl

Scrape a page, search the web, discover URLs, or start a small crawl or batch scrape using the deployment’s Firecrawl account.

## Call

With Crab signed in to Cloud, replace `owner/firecrawl` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/firecrawl scrape '{"url":"https://example.com"}'
```

Returns the page data. Use `search` for up to ten results, `map` for URL discovery, and `crawl` or `batch_scrape` for background jobs.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crab.talk/docs/guides/mcp). Calls use this deployment’s provider credentials.

## Deploy your own

If this page offers **Deploy**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/firecrawl
```

Set `FIRECRAWL_API_KEY` through the copy’s credential setup. Scraped pages and search results consume the connected Firecrawl account’s credits.

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source).

## Usage notes

Crawl and batch submission return a job ID. Call `crawl_status` or `batch_scrape_status` explicitly and pass `skip` from the returned `next` URL for another page. Results retain status, page data and `creditsUsed`.

Search, crawl and batch inputs allow at most ten results, pages or URLs. Map defaults to 1,000 links and accepts a smaller limit. Domain/path filters allow at most 20 entries.

Scrape and batch support Markdown, HTML and links. PDF parsing and charged extraction add-ons are disabled. Agents, monitors and account operations are not included.

Calls do not automatically poll, paginate or retry. A `success: false` response is returned as an error.

[Source and Rust API](https://github.com/crabtalk/crab/tree/main/services/connect/firecrawl) · [Provider documentation](https://docs.firecrawl.dev/api-reference/v2-introduction)
