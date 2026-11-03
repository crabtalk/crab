use crate::monitor::{History, Monitor, Status};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, tool};

/// Add or replace a monitor. Default success is 2xx; checks never follow redirects.
#[tool]
pub fn watch(
    name: String,
    url: String,
    interval_minutes: Option<u64>,
    expected_status: Option<u16>,
) -> Result<Monitor> {
    Monitor::watch(name, url, interval_minutes, expected_status)
}

/// Delete a monitor and its observation history.
#[tool]
pub fn unwatch(name: String) -> Result<bool> {
    Monitor::unwatch(&name)
}

/// List configured URLs and their latest observations.
#[tool(query)]
pub fn monitors() -> Result<Vec<Monitor>> {
    Monitor::all()
}

/// Check one monitor now and retain the observation.
#[tool]
pub fn check(name: String) -> Result<Monitor> {
    Monitor::check(&name)
}

/// Read recorded observations after a cursor.
#[tool(query)]
pub fn history(name: String, after: Option<u64>, limit: Option<u64>) -> Result<History> {
    Monitor::history(&name, after.unwrap_or(0), limit)
}

/// Return names and freshness-aware states without target URLs or errors.
#[tool(query)]
pub fn status() -> Result<Vec<Status>> {
    Monitor::statuses()
}

/// Check up to five due monitors.
#[tool(cron)]
pub fn tick() -> Result<u64> {
    Monitor::tick()
}
