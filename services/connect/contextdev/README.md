# Context.dev

Look up brands, extract styleguides, scrape pages, answer questions, search the web, map URLs and start crawls using the deployment’s Context.dev account.

## Call

With Crab signed in to Cloud, replace `owner/contextdev` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/contextdev brand '{"input":{"body":{"type":"by_domain","domain":"example.com"}}}'
```

Returns the upstream brand profile and request metadata. Brand lookup also accepts names, email, ticker, transaction details or page URLs. Use `styleguide`, `scrape`, `answers`, `search`, `map` or `crawl` for web content.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crab.talk/docs/guides/mcp). Calls use this deployment’s provider credentials.

## Deploy your own

If this page offers **Deploy**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/contextdev
```

Set `CONTEXT_DEV_API_KEY` through the copy’s credential setup. Requests consume the connected Context.dev account’s credits.

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source).

## Usage notes

GET parameters are nested under `input.query`; POST JSON is under `input.body`. Inspect the selected tool’s schema for its supported options.

Responses retain source evidence, partial results and credit metadata. Scrape supports screenshots and products. Pagination, polling and retries are explicit.

ISIN and NAICS brand lookup, people/company/news search, batches, monitors and account administration are not included. Brand industry labels use EIC.

[Source and Rust API](https://github.com/crabtalk/crab/tree/main/services/connect/contextdev) · [Provider documentation](https://docs.context.dev)
