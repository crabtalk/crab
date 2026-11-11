# Upstash Redis

Read and write UTF-8 strings in the deployment’s Upstash Redis database, manage expiry and counters, or run pipelines and transactions.

## Call

With Crab signed in to Cloud, replace `owner/upstash` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

```sh
crab call owner/upstash get '{"key":"report:42"}'
```

Returns the stored string, or `null` when the key is missing. Use `set` to write, `ttl` to inspect expiry, and `increment` for counters.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crab.talk/docs/guides/mcp). Calls use this deployment’s provider credentials and shared state.

## Deploy your own

If this page offers **Deploy**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/upstash
```

Set `UPSTASH_REDIS_REST_URL` to your database’s HTTPS REST origin and set `UPSTASH_REDIS_REST_TOKEN` through credential setup, granting it only to that database’s exact hostname. Read-only tokens support reads; writes need the standard token. Keep the copy private when the database is for your own use.

Set the copy’s variables as needed:

```sh
crab var set upstash UPSTASH_REDIS_REST_URL https://your-database.upstash.io
```

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source).

## Usage notes

`set` replaces a string and its expiry; omit `expiry_seconds` for a persistent value. `ttl` returns seconds, `-1` for persistent keys, and `-2` for missing keys. Binary values are not encoded.

Pipelines can interleave with other clients. Transactions do not interleave, but runtime command errors do not roll back other commands. Inspect every `{value, error?}` entry; a discarded transaction fails the whole call.

Batches can run any Redis command allowed by the token. Calls are not automatically retried, including increments and batches whose effects may already have occurred.

[Rust library integration](https://github.com/crabtalk/crab/blob/main/services/connect/upstash/LIBRARY.md) · [Provider documentation](https://upstash.com/docs/redis/features/restapi)
