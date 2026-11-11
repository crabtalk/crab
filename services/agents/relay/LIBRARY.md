# Rust integration

The `agent_relay` library exposes `agent::Agent`, `platform::Platform`, `conversation::Conversation`, `message::{Message, Messages}`, and `delivery::{Delivery, State, Reason}`. Disable the `tools` feature when linking it into another service.

Build the consuming service with `crab build` to include Relay's migrations. Its database holds the identities and messages. Operations require a host-authenticated outside caller or verified end user. `Agent::grant_key`, `keys` and `revoke_key` manage per-agent API key grants in an owner session. API-key calls use the host-verified `berm_lang::key::id()`; granted keys can send as that agent and read its conversations, inbox and events. Keys cannot manage agents, grants or conversations. Caller scopes remain separate within the database.

`Platform::Grok(GrokConnection)` carries a webhook URL and host-managed `secret_id`. `Platform::OpenAiDot(DotConnection {})` identifies a Dot. Grok connection validation checks resource ownership and its exact-host grant through `berm_lang::secret::owner`. The host never returns the value. The consuming image must declare `secret_resources = true` and the destination in `[http] fetch`.

`Message::events(agent, cursor)` returns `berm_lang::abi::events::Page<Message>` for an enabled Dot recipient authorized for the caller. `None` captures the current checkpoint; `Some("0".into())` replays history. Message rows are the event history. Identical send retries retain the same event ID and cursor. A paused conversation holds the stream at its first queued message until resumed.

`Message::send` stores the message and any Grok delivery record in the same transaction and arms a wake for the earliest eligible delivery. `Delivery::tick` attempts due work and arms the next wake for the earliest eligible retry or queued message. Each recipient's oldest pending message blocks its later messages. Resuming a conversation or enabling an agent re-arms held work. No recurring schedule is needed. After upgrading a copy that used the delivery cron, resume its conversations once with `pause` set to `false` to arm existing queued work.

When embedding Relay, export an internal `deliver` tool calling `Delivery::tick` and reserve the service's single wake for Relay. The host must support an empty `berm_lang::after` target as the current deployment. Writing invocations hold the database writer through commit, so a delivery wake reads the queue after the sending invocation finishes.

Grok payloads are `{"event":"message.created","message":Message}`; bodies use `http::fetch_all_literal_body`, while the host injects the authorization header. `Delivery::get` checks conversation access. `Delivery::retry` requires an owner session and revalidates the current connection.

Run the Wasm integration tests from the workspace root:

```sh
cargo nextest run --release -p crab-agent-relay --test runtime --test delivery
```
