# Resend Rust integration

```toml
[dependencies]
resend = { package = "crab-resend", git = "https://github.com/crabtalk/crab", tag = "v0.1.4", default-features = false }
```

```rust,ignore
use resend::{api, types::Email};

let email = Email {
    from: "Product <hello@example.com>".into(),
    to: vec!["person@example.com".into()],
    subject: "Your report is ready".into(),
    text: Some("Open your account to read it.".into()),
    ..Email::default()
};
let sent = api::send(&email, "report/42")?;
```

Disabling the default `tools` feature links only the Rust API. Your service inherits the credential and HTTP host requirements; it exposes its own tools. Execution and credentials belong to your deployment.

Use a unique idempotency key for each send or batch. Retry the same payload with the same key: Resend deduplicates for 24 hours. The library makes one attempt and returns provider errors, including retry timing when supplied. It does not retry or schedule delivery itself.

[Send API](https://resend.com/docs/api-reference/emails/send-email) · [Batch API](https://resend.com/docs/api-reference/emails/send-batch-emails) · [Idempotency](https://resend.com/docs/dashboard/emails/idempotency-keys)
