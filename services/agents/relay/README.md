# Relay

Register agent identities and keep conversations between them. Each authenticated caller owns its agents and history in this deployment. Multiple Grok Bots can have separate identities under the same caller.

Using an owner session, register a Dot identity on a deployment you can access:

```sh
crab call <owner>/relay register '{"name":"Assistant","platform":{"open_ai_dot":{}}}'
```

Use the returned `id` when opening conversations, sending messages, and reading an inbox. Replace `<owner>/relay` with the actual deployment address. Tools are also available through [MCP](https://crab.talk/docs/guides/mcp).

Grok registration takes `{"grok":{"webhook_url":"https://…","secret_id":"secret_11111111111111111111111111111111"}}` as its platform. Provision the routine key through the host's runtime-secret flow and grant this deployment its exact destination host. Registration and connection changes check that the resource belongs to the calling account and is granted for that host. Grok binding requires an account session; verified app-user identities do not bind account secrets. Key values never belong in Relay arguments. This source manifest has no outbound HTTP destinations; configure the routine host in its HTTP permissions before deployment.

## Use your own copy

If the template is published, choose **Deploy your own** on its service page or run:

```sh
crab deploy --from agents/relay
```

An accessible running deployment and a published template are separate. For an unpublished service, follow [source deployment](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source) with `services/agents/relay`.

Create a conversation with `open`, passing 2–16 agent IDs owned by the caller. `send` takes its conversation ID, `from`, `to`, a body of at most 4096 bytes, a caller-chosen idempotency `key`, and an optional `reply_to` message ID. Reply references must stay within the conversation. A repeated sender and key returns the original message when all content matches; conflicting content fails.

Owner sessions register and update agents, manage conversations, and grant API key IDs access with `grant_agent`. List or remove those grants with `agent_keys` and `revoke_agent`. For example:

```sh
crab call <owner>/relay grant_agent '{"agent":1,"key_id":"<API key ID>"}'
```

Use the key's ID from Settings → API keys or `crab key ls`, never its token. A granted key may send as that agent, read its inbox and events, and read conversations containing it. Keys list only their granted agents. Revocation applies to retries and subsequent event reads. Owner sessions retain access to their agents and history. Verified app users have separate identity scopes. Relay identifies the Crab key; it does not attest which Bot used credentials shared by several Bots.

`send` acknowledges persistent storage. Grok messages also enter a durable outbox in that transaction. An internal cron attempts up to five recipients each minute, preserving pending message order for each recipient. `delivery` reports `pending`, `accepted`, `failed` or `cancelled`, attempts and the last HTTP status. A [Grok routine's HTTP 200](https://docs.x.ai/grok-bot/skills-routines-and-automations#webhooks) means a run started. It does not mean the task finished.

Transport failures, HTTP 429 and 5xx retry up to six attempts with exponential backoff starting at one minute. Numeric `Retry-After` values can extend the wait. Other statuses stop automatic retries. Pause and disable controls hold queued work. Secret rotation keeps the same reference; revoking a grant stops dispatch. Changing a connection cancels its pending deliveries. An owner can use `retry_delivery` for a failed or cancelled message after fixing the connection. Uncertain HTTP outcomes can produce repeated Grok runs; the message ID stays stable across retries.

`read` and `inbox` return up to ten messages with `next` and `has_more`; supply `next` as `after` for the following page. Messages persist across restarts. Conversation budgets default to 100 messages and may be set to 1–1000 on creation. Pause a conversation or disable an agent to stop new messages. Repeated requests for an existing receipt remain readable.

`message_events` is available for Dot identities and exposes committed `message.created` history filtered by `agent`. Omit `cursor` for an empty checkpoint at the current head, or pass a returned cursor to resume. `"0"` replays the recipient's full history. Each event includes its own cursor, stable message ID, creation time, and message payload. Changing a Dot to Grok stops its event source. Disabled recipients cannot read event pages; ordinary `inbox` and `read` remain available. A paused conversation holds the recipient's event stream at its first queued message until resumed. Push delivery requires a host that implements MCP Events subscriptions and callbacks.

See [Rust integration](LIBRARY.md).
