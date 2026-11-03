use crate::{
    record, report,
    site::Site,
    types::{Crawlers, Days, Events, Pages, Recorded, Sources},
};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, tool};

/// Add a website to this private analytics copy.
#[tool]
pub fn add_site(domain: String, name: String) -> Result<Site> {
    Site::add(domain, name)
}

/// List this deployment's tracked sites.
#[tool(query)]
pub fn sites() -> Result<Vec<Site>> {
    Site::all()
}

/// Remove a site and all its recorded data.
#[tool]
pub fn remove_site(site: String) -> Result<bool> {
    Site::remove(&site)
}

/// Record a page view, or a user-agent-labelled AI crawler report.
#[tool]
pub fn record(
    site: String,
    page: String,
    referrer: Option<String>,
    agent: Option<String>,
) -> Result<Recorded> {
    record::record(site, page, referrer, agent)
}

/// Read daily views, daily visitors, AI referrals and crawler requests.
#[tool(query)]
pub fn days(site: String, from: Option<u64>, to: Option<u64>) -> Result<Days> {
    report::days(site, from, to)
}

/// Rank pages by views. Visitors across days are visitor-days.
#[tool(query)]
pub fn pages(
    site: String,
    from: Option<u64>,
    to: Option<u64>,
    limit: Option<u64>,
) -> Result<Pages> {
    report::pages(site, from, to, limit)
}

/// Rank referring hosts by views, excluding internal referrals.
#[tool(query)]
pub fn sources(
    site: String,
    from: Option<u64>,
    to: Option<u64>,
    limit: Option<u64>,
) -> Result<Sources> {
    report::sources(site, from, to, limit)
}

/// Rank AI crawler user-agent labels by reported requests.
#[tool(query)]
pub fn crawlers(site: String, from: Option<u64>, to: Option<u64>) -> Result<Crawlers> {
    report::crawlers(site, from, to)
}

/// Read raw recorded events after a cursor.
#[tool(query)]
pub fn events(site: String, after: Option<u64>, limit: Option<u64>) -> Result<Events> {
    report::events(site, after, limit)
}

/// Remove old raw events, daily client keys and rollups older than 366 days.
#[tool(cron)]
pub fn prune() -> Result<()> {
    record::prune()
}
