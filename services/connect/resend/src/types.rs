use alloc::{string::String, vec::Vec};
use berm_lang::Schema;

/// An email to send. Provide text, HTML, or both.
#[derive(Debug, Clone, Default, Schema)]
pub struct Email {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub cc: Option<Vec<String>>,
    pub bcc: Option<Vec<String>>,
    pub reply_to: Option<Vec<String>>,
}

/// The provider's email identifier; acceptance does not imply delivery.
#[derive(Debug, Clone, Schema)]
pub struct Sent {
    pub id: String,
}

/// An accepted batch, in request order.
#[derive(Debug, Clone, Schema)]
pub struct Batch {
    pub data: Vec<Sent>,
}

/// An email as Resend last recorded it.
#[derive(Debug, Clone, Schema)]
pub struct Delivery {
    pub id: String,
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub created_at: String,
    pub last_event: String,
    pub text: Option<String>,
    pub html: Option<String>,
}
