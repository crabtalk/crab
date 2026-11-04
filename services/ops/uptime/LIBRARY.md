# Rust integration

Link `crab-uptime` with `default-features = false` to omit standalone tool exports.

```rust
use uptime::monitor::Monitor;

let value = Monitor::statuses()?;
```

Build the consumer with `crab build` so its image inherits this library's migrations and HTTP host permissions. Data lives in the consumer's database. Tool access, cron schedules, database reservation and template visibility stay with the consuming service.

Schedule `Monitor::tick` each minute. The library sends GETs to user-defined public HTTP(S) targets.

## Runtime tests

Run from the repository root. Nextest prepares the service image before running the host integration tests.

```sh
cargo nextest run --release -p crab-uptime --test runtime
```
