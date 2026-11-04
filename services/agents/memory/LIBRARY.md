# Rust integration

Link `crab-agent-memory` with `default-features = false` to omit standalone tool exports.

```rust
use agent_memory::memory::Memory;

let value = Memory::get("project", "release-notes")?;
```

Build the consumer with `crab build` so its image inherits this library's migrations. Data lives in the consumer's database. Tool access, cron schedules, database reservation and template visibility stay with the consuming service.

Memories persist until deleted. Callers share the same namespace and key space.

## Runtime tests

Run from the repository root. Nextest prepares the service image before running the host integration tests.

```sh
cargo nextest run --release -p crab-agent-memory --test runtime
```
