# Outbox

Review emails prepared by an assistant before sending them through your deployed copy’s Resend account.

```sh
crab call you/outbox review '{"input":{"from":"Reports <hello@your-verified-domain.com>","to":["reader@example.com"],"subject":"Your report","text":"The report is ready."}}'
```

Replace `you/outbox` with an accessible deployment on a host supporting service requests. The call returns a review link; it does not send the email. Open the link, check the sender, recipients, subject and text, then approve or decline. Connected MCP assistants can use the same `review` tool and share its link. The hosted request flow requires an account owner’s browser approval before execution.

## Deploy your copy

When the [agents/outbox template](https://crabtalk.ai/agents/outbox) is available, choose **Deploy**, or run `crab deploy --from agents/outbox`. Your copy uses its own Resend API key and hosting account. Create a sending key at https://resend.com/api-keys and verify the sender domain in Resend. Supply the key through the deployment’s credential page.

The outbox supports plain text of up to 4000 bytes and 100 lines, one to five recipients, and a subject of up to 200 bytes. It does not accept HTML email.

## Outcomes and recovery

Poll the returned code with `crab request CODE --wait`. A completed request contains Resend’s email ID; provider acceptance does not establish delivery. A failed request retains its review and failure. Reopen its page to resume within the host’s execution window. Every attempt uses the original input and the same provider idempotency key. [Resend deduplicates those sends for 24 hours](https://resend.com/docs/dashboard/emails/idempotency-keys); hosted requests allow new attempts for 30 minutes.

`send` is the owner-session execution endpoint. API keys cannot invoke it successfully. Requests are bound to the reviewed service image and action price; a change requires a new review. Declined and expired requests cannot start execution. Inspect the existing outcome before creating a replacement request after an uncertain failure.

[Service-defined requests](https://crabtalk.ai/docs/guides/service-requests) describes the reusable host contract. [Source](https://github.com/crabtalk/crab/tree/main/services/agents/outbox).
