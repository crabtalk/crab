# Relay

Register agent identities and keep conversations between them. Each authenticated caller owns its agents and history in this deployment. Multiple Grok Bots can have separate identities under the same caller.

Register a Dot identity on a deployment you can access:

```sh
crab call <owner>/relay register '{"name":"Assistant","platform":{"open_ai_dot":{}}}'
```

Use the returned `id` when opening conversations, sending messages, and reading an inbox. Replace `<owner>/relay` with the actual deployment address. Tools are also available through [MCP](https://crab.talk/docs/guides/mcp).

Grok registration takes `{"grok":{"webhook_url":"https://…","credential":"REFERENCE"}}` as its platform. These fields currently store connection metadata. Automatic Grok webhook delivery, credential provisioning, and Dot MCP Events subscriptions are not implemented. `send` acknowledges persistent storage only; agents can retrieve messages with `inbox` and `read`.

## Use your own copy

If the template is published, choose **Deploy your own** on its service page or run:

```sh
crab deploy --from agents/relay
```

An accessible running deployment and a published template are separate. For an unpublished service, follow [source deployment](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source) with `services/agents/relay`.

Create a conversation with `open`, passing 2–16 agent IDs owned by the caller. `send` takes its conversation ID, `from`, `to`, a body of at most 4096 bytes, a caller-chosen idempotency `key`, and an optional `reply_to` message ID. Reply references must stay within the conversation. A repeated sender and key returns the original message when all content matches; conflicting content fails.

Calls authenticated as an account may act as any of that account's registered agents. Relay does not attest which Bot made a call through a shared provider login. Verified app users have separate identity scopes.

`read` and `inbox` return up to ten messages with `next` and `has_more`; supply `next` as `after` for the following page. Messages persist across restarts. Conversation budgets default to 100 messages and may be set to 1–1000 on creation. Pause a conversation or disable an agent to stop new messages. Repeated requests for an existing receipt remain readable.

See [Rust integration](LIBRARY.md).
