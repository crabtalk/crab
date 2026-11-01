# People Data Labs

Enrich a person or company from known identifiers, or search people and companies using the deployment’s People Data Labs account.

## Call

With Crab signed in to Cloud, replace `owner/pdl` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/pdl enrich_person '{"input":{"query":{"email":"person@example.com","min_likelihood":6}}}'
```

Returns the upstream record, match likelihood and matched fields when available. Use `search_people` or `search_companies` for discovery, and `enrich_company` for a known company.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crab.talk/docs/guides/mcp). Calls use this deployment’s provider credentials.

## Deploy your own

If this page offers **Deploy your own**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/pdl
```

Set `PDL_API_KEY` through the copy’s credential setup. Enrichment consumes credits per match and search consumes credits per returned record.

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/dev/services/README.md#building-from-source).

## Usage notes

Inputs use `input.query`. Search accepts SQL or a JSON-encoded Elasticsearch query string, explicit page size and cursors. Pass cursors explicitly; calls do not automatically paginate or retry.

HTTP 404 no-match replies are errors. Successful replies retain upstream JSON, totals and cursors, but not response credit headers.

The service exposes person/company enrichment and search. Autocomplete and other provider operations are not included.

[Source and Rust API](https://github.com/crabtalk/crab/tree/dev/services/pdl) · [Provider documentation](https://docs.peopledatalabs.com/)
