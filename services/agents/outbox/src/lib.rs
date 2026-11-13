#![no_std]
extern crate alloc;

use alloc::{format, string::String, vec::Vec};
use berm_lang::{Caller, Result, Schema, abi::requests::Review, key, tool};
use resend::{
    api,
    types::{Email, Sent},
};

#[derive(Debug, Clone, Schema)]
pub struct Draft {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub text: String,
}

impl Draft {
    pub fn check(&self) -> Result<()> {
        if self.to.is_empty() || self.to.len() > 5 {
            return Err("Choose one to five recipients".into());
        }
        for address in core::iter::once(&self.from).chain(&self.to) {
            if address.trim().is_empty()
                || address.len() > 254
                || address.chars().any(char::is_control)
            {
                return Err("Use email addresses up to 254 bytes on one line".into());
            }
        }
        if self.subject.trim().is_empty()
            || self.subject.len() > 200
            || self.subject.chars().any(char::is_control)
        {
            return Err("Use a subject of 1–200 bytes on one line".into());
        }
        if self.text.trim().is_empty()
            || self.text.len() > 4000
            || self.text.lines().count() > 100
            || self
                .text
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err("Use 1–4000 bytes and at most 100 lines of plain email text".into());
        }
        Ok(())
    }
}

/// Prepare an email for browser review without sending it.
#[tool(request)]
pub fn review(input: Draft) -> Result<Review<Draft>> {
    input.check()?;
    let mut body = format!(
        "    From: {}\n    To: {}\n    Subject: {}\n\n",
        input.from,
        input.to.join(", "),
        input.subject
    );
    for line in input.text.split('\n') {
        body.push_str("    ");
        body.push_str(line);
        body.push('\n');
    }
    let review = Review {
        title: "Send email".into(),
        body,
        tool: "send".into(),
        input,
    };
    review.check()?;
    Ok(review)
}

/// Execute an owner-approved email. The request ID is the provider idempotency key for retries within 24 hours.
#[tool]
pub fn send(request: String, input: Draft) -> Result<Sent> {
    let Some(Caller::Outside(owner)) = berm_lang::caller()? else {
        return Err(berm_lang::deny("An account owner must approve this email"));
    };
    if key::id()?.is_some() || berm_lang::user()?.is_some() {
        return Err(berm_lang::deny(
            "An account owner must approve this email through its request page",
        ));
    }
    if request.is_empty()
        || request.len() > 64
        || !request
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return Err("Use the host's request ID".into());
    }
    input.check()?;
    let email = Email {
        from: input.from,
        to: input.to,
        subject: input.subject,
        text: Some(input.text),
        ..Email::default()
    };
    api::send(&email, &format!("outbox/{owner}/{request}"))
}
