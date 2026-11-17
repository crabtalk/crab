# Upstash Redis

Read and write UTF-8 strings, manage expiry and counters, or run pipelines and transactions from your service using the Upstash Redis Rust library.

Link the `crabkit-upstash` crate as `upstash` in your service.

```rust,ignore
use upstash::api::Redis;

let redis = Redis::new("https://your-database.upstash.io")?;
redis.set("report:42", "ready", Some(3600))?;
let report = redis.get("report:42")?;
let count = redis.increment("reports", 1)?;
```

Your deployment inherits the credential and supplies its database host:

```toml
[credentials.upstash_redis]
hosts = ["your-database.upstash.io"]
```

Pass the endpoint explicitly. `command` also accepts Redis command arguments as strings and returns a JSON value.

[REST API, pipelines and transactions](https://upstash.com/docs/redis/features/restapi)
