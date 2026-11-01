# CompanyEnrich

Enrich company and person records, search for prospects, inspect workforce data, find email addresses, and submit batch enrichment or export jobs.

## Call

With Crab signed in to Cloud, replace `owner/companyenrich` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/companyenrich enrich_company '{"input":{"query":{"domain":"example.com","expand":["workforce"],"waitForEnrichment":false}}}'
```

Returns the provider’s company record with the requested workforce expansion. Use company/person search for discovery, and the matching status tool after submitting a background job.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crab.talk/docs/guides/mcp). Calls use this deployment’s provider credentials.

## Deploy your own

If this page offers **Deploy your own**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/companyenrich
```

Set `COMPANYENRICH_API_KEY` through the copy’s credential setup. Enrichment, search and background jobs consume the connected account’s credits; email enrichment endpoints are beta.

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/dev/services/README.md#building-from-source).

## Usage notes

Responses preserve upstream JSON. Pagination, job polling, retries and result downloads are explicit; submitting a job does not wait for its completion.

Company/person operations, batch jobs, exports, taxonomies, geography and account reads are included. Saved-list CRUD, standalone education lookup and similar-company preview are not included. Education and workforce expansion options on supported tools are retained.

[Source and Rust API](https://github.com/crabtalk/crab/tree/dev/services/companyenrich) · [Provider documentation](https://docs.companyenrich.com)
