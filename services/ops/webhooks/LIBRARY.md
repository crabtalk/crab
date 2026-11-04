# Rust integration

Link `crab-webhook-inbox` with `default-features = false` to omit standalone tool exports.

```rust
use webhook_inbox::delivery::Delivery;

let value = Delivery::get(1)?;
```

Build the consumer with `crab build` so its image inherits this library's migrations. Data lives in the consumer's database. Tool access, cron schedules, database reservation and template visibility stay with the consuming service.

Schedule `Delivery::prune` daily. The signing credential is inherited; expose `Delivery::receive` through the consumer’s own signed webhook tool; do not expose `Delivery::receive` through an unsigned anonymous tool.

## Runtime tests

Run from the repository root. Nextest prepares the service image before running the host integration tests.

```sh
cargo nextest run --release -p crab-webhook-inbox --test runtime
```
