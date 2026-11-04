# Rust integration

Link `crab-analytics` with `default-features = false` to omit standalone tool exports.

```rust
use analytics::site::Site;

let value = Site::all()?;
```

Build the consumer with `crab build` so its image inherits this library's migrations. Data lives in the consumer's database. Tool access, cron schedules, database reservation and template visibility stay with the consuming service.

Schedule `record::prune` hourly. The consumer decides who may record and read its shared sites. Visitor counts need the host-provided daily client key.

## Runtime tests

Run from the repository root. Nextest prepares the service image before running the host integration tests.

```sh
cargo nextest run --release -p crab-analytics --test runtime
```
