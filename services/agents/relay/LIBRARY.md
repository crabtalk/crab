# Rust integration

The `agent_relay` library exposes `agent::{Agent, Contact, Contacts}`, `platform::Platform`, `conversation::Conversation`, `message::{Message, Messages}`, and `delivery::{Delivery, Reason}`. Disable the `tools` feature when linking it into another service.

Build the consuming service with `crab build`. Declare `[storage.caller]` using the Relay caller migrations for agent identities, conversations and messages, and declare shared `[storage]` using the wake migrations. The host selects caller storage from the authenticated account. Owner sessions manage profiles and explicit conversations.

`Agent::register(handle, name, platform)` defaults to `Platform::Inbox`. Handles are unique and case-insensitive within an account. A registration retry with the same profile reuses the identity. `Message::address` opens or reuses a direct conversation and `Message::reply` sends as the original recipient in that conversation.

`Message::inbox(handle, after, limit)` returns unread messages from the agent’s stored read position. Pass the last processed message’s ID to advance the position. `Message::incoming` reads incoming history from an explicit cursor. An enabled `Platform::OpenAiDot(DotConnection {})` can also use `Message::events`, where `None` captures the current checkpoint and `Some("0".into())` replays from the beginning. A paused conversation holds the event stream at its first queued message until resumed.

`Platform::Grok(GrokConnection)` carries a Cursor automation URL and a host-managed `secret_id`. The consuming image declares `secret_resources = true` and `[http] fetch = ["api2.cursor.sh"]`. Registration binds the granted resource to the deployment for scheduled delivery. Existing connections require a platform update to bind their resource. The host supplies the Bearer value to the fetch.

Export an internal `deliver` tool calling `Delivery::tick` and reserve the service’s wake for Relay. The shared outbox keeps one entry per agent. `Delivery::tick` only uses shared storage, and sends up to five due wakes. A wake posts `{"message":"@handle has new messages"}`. `Delivery::get(handle)` reads a connection’s pending wake or failure. A transient failure is retried up to six attempts. Submitting a validated platform again resets a failing connection.

Run the Wasm integration tests from the workspace root:

```sh
cargo nextest run -p crab-agent-relay
```
