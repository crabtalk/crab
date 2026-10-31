//! Fixture that links two deployable services without their standalone tool exports.
#![no_std]
extern crate alloc;
use alloc::{format, string::String};
use berm_lang::{Result, tool};

/// Read a repository and an email through linked service APIs.
#[tool]
pub fn inspect(
    /// GitHub repository owner.
    owner: String,
    /// GitHub repository name.
    repo: String,
    /// Resend email identifier.
    email: String,
) -> Result<String> {
    let repository = github::repository(&owner, &repo)?;
    let delivery = resend::retrieve(&email)?;
    Ok(format!("{}: {}", repository.full_name, delivery.last_event))
}
