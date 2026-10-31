//! Fixture that links deployable services without their standalone tool exports.
#![no_std]
extern crate alloc;
use alloc::{format, string::String};
use berm_lang::{Result, tool};
use github::api as github_api;
use posthog::{
    api::PostHog,
    types::{Evaluation, Region},
};
use release_watch::api as release_watch_api;
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
    let watched = release_watch_api::watches()?;
    let repository = github_api::repository(&owner, &repo)?;
    let delivery = resend_api::retrieve(&email)?;
    let cache = Redis::new(&redis_url)?.get("status")?;
    let flags = PostHog::new(Region::Us).flags(&Evaluation {
        distinct_id,
        ..Evaluation::default()
    })?;
    Ok(format!(
        "{}: {}; cache={cache:?}; flags={}",
        repository.full_name,
        delivery.last_event,
        flags.flags.len() + watched.len()
    ))
}
