use crate::{
    record, report,
    site::Site,
    types::{Crawlers, Days, Events, Pages, Recorded, Sources},
};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, tool};

/// Add website
///
/// Add a website to this private analytics copy.
#[tool(
    catalog = "web.analytics.add_site",
    budget(cpu = 100000, read = 500000, written = 2, memory = 16)
)]
pub fn add_site(domain: String, name: String) -> Result<Site> {
    Site::add(domain, name)
}

/// List websites
///
/// List this deployment's tracked sites.
#[tool(
    query,
    catalog = "web.analytics.sites",
    budget(cpu = 100000, read = 500000, written = 1, memory = 16)
)]
pub fn sites() -> Result<Vec<Site>> {
    Site::all()
}

/// Remove website
///
/// Remove a site and all its recorded data.
#[tool(
    catalog = "web.analytics.remove_site",
    budget(cpu = 100000, read = 500000, written = 1000000, memory = 16)
)]
pub fn remove_site(site: String) -> Result<bool> {
    Site::remove(&site)
}

/// Record page view
///
/// Record a page view, or a user-agent-labelled AI crawler report.
#[tool(
    access = "anonymous",
    catalog = "web.analytics.record",
    budget(cpu = 100000, read = 500000, written = 20, memory = 16)
)]
pub fn record(
    site: String,
    page: String,
    referrer: Option<String>,
    agent: Option<String>,
) -> Result<Recorded> {
    record::record(site, page, referrer, agent)
}

/// Daily analytics
///
/// Read daily views, daily visitors, AI referrals and crawler requests.
#[tool(
    query,
    catalog = "web.analytics.days",
    budget(cpu = 100000, read = 500000, written = 1, memory = 16)
)]
pub fn days(site: String, from: Option<u64>, to: Option<u64>) -> Result<Days> {
    report::days(site, from, to)
}

/// Popular pages
///
/// Rank pages by views. Visitors across days are visitor-days.
#[tool(
    query,
    catalog = "web.analytics.pages",
    budget(cpu = 100000, read = 500000, written = 1, memory = 16)
)]
pub fn pages(
    site: String,
    from: Option<u64>,
    to: Option<u64>,
    limit: Option<u64>,
) -> Result<Pages> {
    report::pages(site, from, to, limit)
}

/// Referring websites
///
/// Rank referring hosts by views, excluding internal referrals.
#[tool(
    query,
    catalog = "web.analytics.sources",
    budget(cpu = 100000, read = 500000, written = 1, memory = 16)
)]
pub fn sources(
    site: String,
    from: Option<u64>,
    to: Option<u64>,
    limit: Option<u64>,
) -> Result<Sources> {
    report::sources(site, from, to, limit)
}

/// AI crawler requests
///
/// Rank AI crawler user-agent labels by reported requests.
#[tool(
    query,
    catalog = "web.analytics.crawlers",
    budget(cpu = 100000, read = 500000, written = 1, memory = 16)
)]
pub fn crawlers(site: String, from: Option<u64>, to: Option<u64>) -> Result<Crawlers> {
    report::crawlers(site, from, to)
}

/// Recorded page views
///
/// Read raw recorded events after a cursor.
#[tool(
    query,
    catalog = "web.analytics.events",
    budget(cpu = 100000, read = 500000, written = 1, memory = 16)
)]
pub fn events(site: String, after: Option<u64>, limit: Option<u64>) -> Result<Events> {
    report::events(site, after, limit)
}

/// Prune
///
/// Remove old raw events, daily client keys and rollups older than 366 days.
#[tool(
    cron,
    access = "internal",
    budget(cpu = 100000, read = 1000000, written = 1000000, memory = 16)
)]
pub fn prune() -> Result<()> {
    record::prune()
}
