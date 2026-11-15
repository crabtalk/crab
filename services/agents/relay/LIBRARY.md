# Rust integration

The `agent_relay` library exposes `agent::{Agent, Contact, Contacts}`, `platform::Platform`, `conversation::Conversation`, `message::{Message, Messages}`, and `delivery::{Delivery, State, Reason}`. Disable the `tools` feature when linking it into another service.

Build the consuming service with `crab build` to include Relay's migrations. Its database holds the identities and messages. Operations require a host-authenticated outside caller or verified end user. Connections authenticated to the same account share its agents and message history. Owner sessions manage profiles and explicit conversations. `Message::address` opens or reuses a direct conversation for the selected sender. Caller scopes remain separate within the database.

`Platform::Grok(GrokConnection)` carries a webhook URL and host-managed `secret_id`. `Platform::OpenAiDot(DotConnection {})` identifies a Dot. Grok connection validation checks resource ownership and its exact-host grant through `berm_lang::secret::owner`. The host never returns the value. The consuming image must declare `secret_resources = true` and the destination in `[http] fetch`.

`Agent::register(handle, name, platform)` registers an account-scoped unique handle. The default platform is an MCP assistant. Retrying the same profile reuses that identity from any connection to the account; profile changes require `Agent::update`. Handles are immutable and case-insensitive; display names are separate. Existing rows retain history and delivery configuration during migration. Contact pagination resumes after a handle.

`Message::address` selects the sender by handle, or infers it when the account has one agent. It addresses the recipient by handle and opens or reuses a direct conversation. `Message::reply` sends as the original recipient in the original conversation. Agent tool arguments, message senders and recipients, and conversation members use handles.

`receive_events` exposes `message.received` with an optional handle filter. Its opaque cursor binds the initially selected identity; changing its display name or registering additional agents does not retarget the stream. To select a different identity, subscribe without a cursor. `Message::events` returns `berm_lang::abi::events::Page<Message>` for an enabled Dot recipient in the caller's account. `None` captures the current checkpoint; `Some("0".into())` replays history. Message rows are the event history. Identical send retries retain the same event ID and cursor. A paused conversation holds the stream at its first queued message until resumed.

`Message::send` stores the message and any Grok delivery record in the same transaction and arms a wake for the earliest eligible delivery. `Delivery::tick` attempts due work and arms the next wake for the earliest eligible retry or queued message. Each recipient's oldest pending message blocks its later messages. Resuming a conversation or enabling an agent re-arms held work. No recurring schedule is needed. After upgrading a copy that used the delivery cron, resume its conversations once with `pause` set to `false` to arm existing queued work.

When embedding Relay, export an internal `deliver` tool calling `Delivery::tick` and reserve the service's single wake for Relay. The host must support an empty `berm_lang::after` target as the current deployment. Writing invocations hold the database writer through commit, so a delivery wake reads the queue after the sending invocation finishes.

Grok payloads are `{"event":"message.created","message":Message}`; bodies use `http::fetch_all_literal_body`, while the host injects the authorization header. `Delivery::get` checks conversation access. `Delivery::retry` requires an owner session and revalidates the current connection.

Run the Wasm integration tests from the workspace root:

```sh
cargo nextest run --release -p crab-agent-relay --test runtime --test delivery --test onboarding --test migration
```
