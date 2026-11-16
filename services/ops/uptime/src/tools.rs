use crate::monitor::{History, Monitor, Status};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, tool};

/// Watch URL
///
/// Add or replace a monitor. Default success is 2xx; checks never follow redirects.
#[tool(
    catalog = "uptime.watch",
    budget(cpu = 100000, read = 20000, written = 5000, memory = 16)
)]
pub fn watch(
    name: String,
    url: String,
    interval_minutes: Option<u64>,
    expected_status: Option<u16>,
) -> Result<Monitor> {
    Monitor::watch(name, url, interval_minutes, expected_status)
}

/// Remove monitor
///
/// Delete a monitor and its observation history.
#[tool(
    catalog = "uptime.unwatch",
    budget(cpu = 100000, read = 20000, written = 5000, memory = 16)
)]
pub fn unwatch(name: String) -> Result<bool> {
    Monitor::unwatch(&name)
}

/// List monitors
///
/// List configured URLs and their latest observations.
#[tool(
    query,
    catalog = "uptime.monitors",
    budget(cpu = 100000, read = 20000, written = 5000, memory = 16)
)]
pub fn monitors() -> Result<Vec<Monitor>> {
    Monitor::all()
}

/// Check now
///
/// Check one monitor now and retain the observation.
#[tool(
    catalog = "uptime.check",
    budget(cpu = 100000, read = 20000, written = 5000, memory = 16)
)]
pub fn check(name: String) -> Result<Monitor> {
    Monitor::check(&name)
}

/// Read uptime history
///
/// Read recorded observations after a cursor.
#[tool(
    query,
    catalog = "uptime.history",
    budget(cpu = 100000, read = 20000, written = 5000, memory = 16)
)]
pub fn history(name: String, after: Option<u64>, limit: Option<u64>) -> Result<History> {
    Monitor::history(&name, after.unwrap_or(0), limit)
}

/// Service status
///
/// Return names and freshness-aware states without target URLs or errors.
#[tool(
    query,
    access = "anonymous",
    catalog = "uptime.status",
    budget(cpu = 100000, read = 20000, written = 5000, memory = 16)
)]
pub fn status() -> Result<Vec<Status>> {
    Monitor::statuses()
}

/// Check up to five due monitors
///
/// Check up to five due monitors.
#[tool(
    cron,
    access = "internal",
    budget(cpu = 100000, read = 200000, written = 25000, memory = 16)
)]
pub fn tick() -> Result<u64> {
    Monitor::tick()
}
