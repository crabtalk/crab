# Rust integration

Link `crab-webhook-inbox` with `default-features = false` to omit standalone tool exports.

```rust
use webhook_inbox::delivery::Delivery;

let value = Delivery::get(1)?;
```

Build the consumer with `crab build` so its image inherits this library's migrations. Data lives in the consumer's database. Tool access, cron schedules, database reservation and template visibility stay with the consuming service.

Schedule `Delivery::prune` daily. The signing credential is inherited; expose `Delivery::receive` through the consumer’s own signed webhook tool; do not expose `Delivery::receive` through an unsigned anonymous tool.

## Runtime tests

Build the image, then run the ignored host integration tests explicitly from the repository root. These require the prepared image and do not build inside a test:

```sh
crab build services/ops/webhooks
BERM_TEST_IMAGE_CRAB_WEBHOOK_INBOX="$PWD/target/berm/webhook_inbox.wasm" cargo nextest run -p crab-webhook-inbox --test runtime --run-ignored only
```
