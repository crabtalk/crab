//! Fixture that links deployable services without their standalone tool exports.
#![no_std]
extern crate alloc;
use alloc::{format, string::String};
use berm_lang::{Result, tool};
use github::api as github_api;
use memory::memory::Memory;
use posthog::{
    api::PostHog,
    types::{Evaluation, Region},
};
use resend::api as resend_api;
use upstash::api::Redis;

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
    let memory = Memory::get("fixture", "status")?;
    let repository = github_api::repository(&owner, &repo)?;
    let delivery = resend_api::retrieve(&email)?;
    let cache = Redis::new(&redis_url)?.get("status")?;
    let flags = PostHog::new(Region::Us).flags(&Evaluation {
        distinct_id,
        ..Evaluation::default()
    })?;
    Ok(format!(
        "{}: {}; cache={cache:?}; flags={}; memory={:?}",
        repository.full_name,
        delivery.last_event,
        flags.flags.len(),
        memory.map(|entry| entry.content)
    ))
}
