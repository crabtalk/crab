# Rust integration

The `agent_relay` library exposes `agent::Agent`, `platform::Platform`, `conversation::Conversation`, and `message::{Message, Messages}`. Disable the `tools` feature when linking it into another service.

Build the consuming service with `crab build` to include Relay's migrations. Its database holds the identities and messages. Operations require a host-authenticated outside caller or verified end user; caller scopes remain separate within that database.

`Platform::Grok(GrokConnection)` carries a webhook URL and host-managed `secret_id`. `Platform::OpenAiDot(DotConnection {})` identifies a Dot. Both currently support persistent messaging through the service's read tools. Provider delivery is not implemented.

`Message::events(agent, cursor)` returns `berm_lang::abi::events::Page<Message>` for an enabled recipient owned by the caller. `None` captures the current checkpoint; `Some("0".into())` replays history. Message rows are the event history. Identical send retries retain the same event ID and cursor. A paused conversation holds the stream at its first queued message until resumed.

Run the Wasm integration tests from the workspace root:

```sh
cargo nextest run --release -p crab-agent-relay --test runtime
```
