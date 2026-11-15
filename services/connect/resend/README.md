# Resend

Send email through the deployment’s Resend account, send a batch of up to 100 emails, and check delivery events.

## Call

With Crab signed in to Cloud, replace `owner/resend` with the address of a running deployment you can access. If this page only offers a template, deploy a copy first.

Replace the sender with a domain verified in the deployment’s Resend account and choose the intended recipient. This call sends an email.

```sh
crab call owner/resend send '{"email":{"from":"Reports <reports@your-verified-domain.com>","to":["recipient@example.com"],"subject":"Your report is ready","text":"Your report is available in your account."},"idempotency_key":"report-42"}'
```

Returns an email ID. Acceptance does not confirm delivery; call `retrieve` with that ID to read the latest delivery event. Use `send_batch` for a batch.

On a callable deployment, you can also open a tool on this page to call it, or connect your agent through [CrabTalk MCP](https://crabtalk.ai/docs/guides/mcp). Calls use this deployment’s provider credentials.

## Deploy your own

If this page offers **Deploy**, use it to create a private copy with your own credentials and state. The CLI equivalent is:

```sh
crab deploy --from owner/resend
```

Set `RESEND_API_KEY` through the copy’s credential setup and verify your sending domain in Resend. Sending-only keys support `send` and `send_batch`; `retrieve` needs full access. Keep the copy private when its sending account is for your own use.

Template deployment needs a published image. To build a modified version or deploy when no template is published, use the [source instructions](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source).

## Usage notes

Use a unique idempotency key for each send or batch. Reuse the same key only with the same payload when retrying; Resend deduplicates for 24 hours.

Calls make one attempt. Provider errors retain retry timing when supplied; the service does not retry or schedule delivery.

[Rust library integration](https://github.com/crabtalk/crab/blob/main/services/connect/resend/LIBRARY.md) · [Provider documentation](https://resend.com/docs/api-reference/emails/send-email)
