# Rust integration

Link `crab-agent-memory` with `default-features = false` to omit standalone tool exports.

```rust
use agent_memory::memory::Memory;

let value = Memory::get("project", "release-notes")?;
```

Build the consumer with `crab build` so its image inherits this library's migrations. Data lives in the consumer's database. Tool access, cron schedules, database reservation and template visibility stay with the consuming service.

Memories persist until deleted. Callers share the same namespace and key space.

## Runtime tests

Build the image, then run the ignored host integration tests explicitly from the repository root. These require the prepared image and do not build inside a test:

```sh
crab build services/agents/memory
BERM_TEST_IMAGE_CRAB_AGENT_MEMORY="$PWD/target/berm/agent_memory.wasm" cargo nextest run -p crab-agent-memory --test runtime --run-ignored only
```
