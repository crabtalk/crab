# Rust integration

Link `crab-uptime` with `default-features = false` to omit standalone tool exports.

```rust
use uptime::monitor::Monitor;

let value = Monitor::statuses()?;
```

Build the consumer with `crab build` so its image inherits this library's migrations and HTTP host permissions. Data lives in the consumer's database. Tool access, cron schedules, database reservation and template visibility stay with the consuming service.

Schedule `Monitor::tick` each minute. The library sends GETs to user-defined public HTTP(S) targets.

## Runtime tests

Build the image, then run the ignored host integration tests explicitly from the repository root. These require the prepared image and do not build inside a test:

```sh
crab build services/ops/uptime
BERM_TEST_IMAGE_CRAB_UPTIME="$PWD/target/berm/uptime.wasm" cargo nextest run -p crab-uptime --test runtime --run-ignored only
```
