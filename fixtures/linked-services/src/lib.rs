//! Fixture that links kits into one service without their own tool exports.
#![no_std]
extern crate alloc;
use alloc::{format, string::String};
// Linked for its kit only; the build test expects crabkit-auth among the kits.
use auth as _;
use berm_lang::{Result, tool};
use github::api as github_api;
use upstash::api::Redis;

/// Read provider data through linked kit APIs
///
/// Read provider data through linked kit APIs.
#[tool]
pub fn inspect(
    /// GitHub repository owner.
    owner: String,
    /// GitHub repository name.
    repo: String,
    /// Upstash database HTTPS origin.
    redis_url: String,
) -> Result<String> {
    let repository = github_api::repository(&owner, &repo)?;
    let cache = Redis::new(&redis_url)?.get("status")?;
    Ok(format!("{}: cache={cache:?}", repository.full_name))
}
