# Rust integration

Link `crab-analytics` with `default-features = false` to omit standalone tool exports.

```rust
use analytics::site::Site;

let value = Site::all()?;
```

Build the consumer with `crab build` so its image inherits this library's migrations. Data lives in the consumer's database. Tool access, cron schedules, database reservation and template visibility stay with the consuming service.

Schedule `record::prune` hourly. The consumer decides who may record and read its shared sites. Visitor counts need the host-provided daily client key.

## Runtime tests

Build the image, then run the ignored host integration tests explicitly from the repository root. These require the prepared image and do not build inside a test:

```sh
crab build services/web/analytics
BERM_TEST_IMAGE_CRAB_ANALYTICS="$PWD/target/berm/analytics.wasm" cargo nextest run -p crab-analytics --test runtime --run-ignored only
```

Run the browser collector unit tests with `node --test services/web/analytics/tests/tracker.cjs`. They use Node's VM and do not launch a browser.
