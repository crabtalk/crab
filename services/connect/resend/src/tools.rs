use crate::{
    api,
    types::{Batch, Delivery, Email, Sent},
};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, tool};

/// Send email
///
/// Send an email through the deployment's Resend account. Acceptance does not imply delivery.
#[tool(idempotency = "24h")]
pub fn send(
    /// Sender, recipients, subject and text or HTML body.
    email: Email,
    /// A unique key for this send; reuse with the same payload for retries within 24 hours.
    idempotency_key: String,
) -> Result<Sent> {
    api::send(&email, &idempotency_key)
}

/// Send batch emails
///
/// Send up to 100 emails in one Resend request.
#[tool(idempotency = "24h")]
pub fn send_batch(
    /// Emails in delivery order.
    emails: Vec<Email>,
    /// A unique key for this batch; reuse only with the same payload.
    idempotency_key: String,
) -> Result<Batch> {
    api::send_batch(&emails, &idempotency_key)
}

/// Read email delivery
///
/// Read an email's current delivery event from Resend.
#[tool]
pub fn retrieve(
    /// The identifier returned by send or send_batch.
    id: String,
) -> Result<Delivery> {
    api::retrieve(&id)
}
