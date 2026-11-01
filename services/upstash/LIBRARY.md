# Upstash Redis Rust integration

```toml
[dependencies]
upstash = { package = "crab-upstash", git = "https://github.com/crabtalk/crab", branch = "dev", default-features = false }
```

```rust,ignore
use upstash::api::Redis;

let redis = Redis::new("https://your-database.upstash.io")?;
redis.set("report:42", "ready", Some(3600))?;
let report = redis.get("report:42")?;
let count = redis.increment("reports", 1)?;
```

Your deployment inherits the credential and Upstash HTTP permission. Pass the endpoint explicitly; deployment variables and tool exports are not inherited. `command` also accepts Redis command arguments as strings and returns a JSON value.

[REST API, pipelines and transactions](https://upstash.com/docs/redis/features/restapi)
