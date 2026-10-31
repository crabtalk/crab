# Upstash Redis

Read and write an existing Upstash Redis database through HTTPS. Tools: `get`, `set`, `delete`, `expire`, `ttl`, `increment`, `pipeline`, `transaction`.

## Deploy

From this directory, run `crab build`, then `crab deploy`. Set `UPSTASH_REDIS_REST_URL` to the database's HTTPS REST origin from [Upstash Console](https://console.upstash.com). Set the host-managed `UPSTASH_REDIS_REST_TOKEN`, granting it to that exact database hostname. The token never enters guest memory. Read-only tokens support reads; writes need the standard token.

Agents call the deployed tools through MCP. Keep the deployment private when its database should only be used by your agents. Batch tools can run arbitrary Redis commands allowed by the token.

`set` replaces a string and its expiry; omit expiry for a persistent value. `ttl` returns seconds, -1 for a persistent key, or -2 for a missing key. Keys and values go in JSON request bodies, not URLs. This API handles UTF-8 string values; it does not encode binary values.

Pipelines can interleave with other clients. Transactions do not interleave, but runtime command errors do **not** roll back other commands. Both return ordered `{value, error?}` entries, including null for missing values; inspect every error. A discarded transaction fails the whole call. Calls are never automatically retried, particularly increments and batches whose effects may already have occurred.

## Use in another service

```toml
[dependencies]
upstash = { package = "crab-upstash", git = "https://github.com/crabtalk/crab", branch = "dev", default-features = false }
```

```rust,ignore
let redis = upstash::Redis::new("https://your-database.upstash.io")?;
redis.set("report:42", "ready", Some(3600))?;
let report = redis.get("report:42")?;
let count = redis.increment("reports", 1)?;
```

Your deployment inherits the credential and Upstash HTTP permission. Pass the endpoint explicitly; deployment variables and tool exports are not inherited. `command` also accepts Redis command arguments as strings and returns a JSON value.

[REST API, pipelines and transactions](https://upstash.com/docs/redis/features/restapi)
