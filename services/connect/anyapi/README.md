# AnyAPI

Look up social profiles and companies, search posts and ads, read reviews and maps, or extract web content using the deployment’s AnyAPI account.

## Call

With Crab signed in to Cloud, replace `owner/anyapi` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/anyapi linkedin_company '{"input":{"body":{"url":"https://www.linkedin.com/company/stripe"},"query":{"max_cost_usd":"0.05"}}}'
```

Returns the provider’s `output`, `costUsd`, item count and available metadata. A result with `output.found: false` is still a successful lookup response.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crab.talk/docs/guides/mcp). Calls use this deployment’s provider credentials.

## Deploy your own

If this page offers **Deploy your own**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/anyapi
```

Set `ANYAPI_API_KEY` through the copy’s credential setup. Requests spend the connected AnyAPI wallet balance; charges vary by operation and selected source.

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/dev/services/README.md#building-from-source).

## Usage notes

POST payloads go under `input.body`; optional response filters and `max_cost_usd` go under `input.query`, and `Idempotency-Key` under `input.headers`. Inspect the chosen tool’s schema for source controls and supported fields.

Response filters do not reduce the upstream charge. `max_cost_usd` bounds eligible source prices. Pass returned cursors explicitly for another page; calls do not automatically retry.

The service covers selected Facebook, Google, Instagram, LinkedIn, Maps, Reddit, TikTok, TikTok Shop, X, YouTube and web extraction operations. Discovery and account endpoints are not included.

[Source and Rust API](https://github.com/crabtalk/crab/tree/dev/services/anyapi) · [Provider documentation](https://getanyapi.com/docs/quickstart)
