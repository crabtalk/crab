use crate::{
    api,
    types::{Changes, Refreshed, Status, Watch},
};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, tool};

/// Follow an installed package with an explicitly supplied GitHub repository mapping.
#[tool]
pub fn follow(
    /// Watch name, exact package version, and repository owner and name.
    watch: Watch,
) -> Result<()> {
    api::follow(&watch)
}

/// Stop checking a watch. Retained history is preserved.
#[tool]
pub fn unfollow(
    /// Name supplied when following.
    name: String,
) -> Result<()> {
    api::unfollow(&name)
}

/// Read active watches, their last successful snapshots and current failures.
#[tool(query)]
pub fn watches() -> Result<Vec<Status>> {
    api::watches()
}

/// Read retained baseline, change, failure and recovery observations using a cursor.
#[tool(query)]
pub fn changes(
    /// Last cursor already read; zero starts at the beginning.
    after: u64,
    /// Number of observations, 1 to 100; default 20.
    limit: Option<u64>,
) -> Result<Changes> {
    api::changes(after, limit)
}

/// Check one watch now. A newer release is evidence to review, not proof of a fix.
#[tool]
pub fn refresh(
    /// Name supplied when following.
    name: String,
) -> Result<Status> {
    api::refresh(&name)
}

/// Check up to five watches due for their hourly refresh.
#[tool(cron)]
pub fn tick() -> Result<Refreshed> {
    api::refresh_due()
}
