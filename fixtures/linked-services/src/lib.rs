//! Fixture that links deployable services without their standalone tool exports.
#![no_std]
extern crate alloc;
use alloc::{format, string::String};
use berm_lang::{Result, tool};

/// Read provider data through linked service APIs.
#[tool]
pub fn inspect(
    /// GitHub repository owner.
    owner: String,
    /// GitHub repository name.
    repo: String,
    /// Resend email identifier.
    email: String,
    /// Upstash database HTTPS origin.
    redis_url: String,
    /// PostHog user identifier.
    distinct_id: String,
) -> Result<String> {
    let repository = github::repository(&owner, &repo)?;
    let delivery = resend::retrieve(&email)?;
    let cache = upstash::Redis::new(&redis_url)?.get("status")?;
    let flags = posthog::PostHog::new(posthog::Region::Us).flags(&posthog::Evaluation {
        distinct_id,
        ..posthog::Evaluation::default()
    })?;
    Ok(format!(
        "{}: {}; cache={cache:?}; flags={}",
        repository.full_name,
        delivery.last_event,
        flags.flags.len()
    ))
}
