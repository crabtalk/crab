# DataForSEO

Get keyword metrics, backlinks, rankings and search results, inspect sites and business data, or submit and retrieve supported DataForSEO tasks.

## Call

With Crab signed in to Cloud, replace `owner/dataforseo` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/dataforseo keyword_volume '{"input":{"body":[{"keywords":["rust hosting","wasm hosting"],"location_code":2840,"language_code":"en"}]}}'
```

Returns the upstream task envelope, including results, task IDs and costs. Live tools return their immediate results; task-submission tools return IDs for explicit retrieval.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crab.talk/docs/guides/mcp). Calls use this deployment’s provider credentials.

## Deploy your own

If this page offers **Deploy your own**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/dataforseo
```

Set `DATAFORSEO_AUTH_TOKEN` through credential setup to the Base64 encoding of `API_LOGIN:API_PASSWORD`, without the `Basic ` prefix. Use the API credentials from your DataForSEO account, not the account password. Requests spend the connected DataForSEO balance.

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/dev/services/README.md#building-from-source).

## Usage notes

POST tools take an array of task objects under `input.body`; GET path parameters go under `input.path`. Live endpoints accept one task per call; Standard task submission limits depend on the tool.

Envelope and task failures return errors containing the original response, including successful tasks in a mixed result. Created tasks and explicitly partial results remain responses.

Calls do not poll, paginate or retry automatically. Google Events Live Advanced and Pinterest Live are not included.

[Source and Rust API](https://github.com/crabtalk/crab/tree/dev/services/dataforseo) · [Provider documentation](https://docs.dataforseo.com/v3/)
