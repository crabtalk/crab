# Webhook inbox

Keep signed JSON events until your agents read and acknowledge them. Each private copy owns its inbox, deduplication records and execution bill.

## Deploy and connect a sender

If this listing offers **Deploy**, create a private copy:

```sh
crab deploy --from owner/webhooks
crab secret request you/webhooks --for "Verify events sent to this inbox"
```

Replace `owner` with the template publisher and `you` with your deploying account. Complete the signing-secret request. The sender uses [Standard Webhooks](https://www.standardwebhooks.com) and posts to:

```text
https://api.crabtalk.ai/services/you/webhooks/webhooks/receive
```

Its JSON body must be an envelope with `source`, `id` and `payload`, for example `{"source":"builds","id":"build-123","payload":{"state":"passed"}}`. Cloud verifies the signature before the event reaches this service. Arbitrary GitHub or Stripe webhooks need an adapter for their signature scheme and this envelope; this is not a universal raw HTTP receiver.

## Process events

```sh
crab call you/webhooks events '{"pending_only":true}'
crab call you/webhooks get '{"seq":1}'
crab call you/webhooks acknowledge '{"seq":1}'
```

The list returns metadata, `next` and `has_more`; pass `next` as `after` to continue. Fetch one event's JSON payload with `get`. Agents can share the copy through [MCP](https://crabtalk.ai/docs/guides/mcp).

A repeated `(source, id)` with identical payload is acknowledged without another event. Reusing it with different payload fails. Acknowledging is idempotent and keeps the event. There is no exclusive worker claim: agents must coordinate processing, and external effects need their own idempotency key.

## Limits

Sources are 1–64 bytes, ids 1–256 bytes and serialized payloads at most 16 KiB. Lists return at most 100 metadata records. The copy holds at most 10000 events within a 64 MiB database. Full inboxes fail before storing new events; senders must retry failures.

Daily cleanup deletes events older than thirty days, including unacknowledged events. `delete` removes one immediately. Either operation also removes its deduplication record, so that id can be accepted again. Keep the deployment private so other accounts cannot read or acknowledge events.

[Cloud webhook setup](https://crabtalk.ai/docs/guides/webhooks) · [Source deployment](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source) · [Rust API](https://github.com/crabtalk/crab/blob/main/services/ops/webhooks/LIBRARY.md)
