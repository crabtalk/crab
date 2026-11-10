# Rust integration

The `agent_relay` library exposes `agent::Agent`, `platform::Platform`, `conversation::Conversation`, and `message::{Message, Messages}`. Disable the `tools` feature when linking it into another service.

Build the consuming service with `crab build` to include Relay's migrations. Its database holds the identities and messages. Operations require a host-authenticated outside caller or verified end user; caller scopes remain separate within that database.

`Platform::Grok(GrokConnection)` carries a webhook URL and credential reference. `Platform::OpenAiDot(DotConnection {})` identifies a Dot. Both currently support persistent messaging through the service's read tools. Provider delivery is not implemented.

Run the Wasm integration tests from the workspace root:

```sh
cargo nextest run --release -p crab-agent-relay --test runtime
```
