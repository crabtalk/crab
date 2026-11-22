use crate::{
    account::Account,
    channels::{By, Channels, Model},
    claim::Claim,
    drain,
    funnel::Funnel,
    journey::Journey,
    period::Window,
    stats::{Interval, Stats},
    stripe::{Stripe, Synced},
    track::{self, Tracked, Utm},
};
use alloc::{string::String, vec::Vec};
use berm_lang::{Result, tool};

/// Claim a website
///
/// Publish the returned token at its URL and call again to verify ownership.
#[tool(
    access = "accounts",
    catalog = "web.analytics-claim",
    budget(written = 55000, memory = 32)
)]
pub fn claim(domain: String) -> Result<Claim> {
    let mut account = Account::calling()?;
    drain::run(&account)?;
    Claim::check(&domain, &mut account)
}

/// Track an anonymous website visit
///
/// Record a view or site event for a verified domain.
#[tool(
    access = "anonymous",
    catalog = "web.analytics-track",
    budget(memory = 16)
)]
#[allow(clippy::too_many_arguments)]
pub fn track(
    domain: String,
    visitor: Option<String>,
    session: String,
    path: String,
    referrer: Option<String>,
    utm: Option<Utm>,
    event: Option<String>,
    value: Option<i64>,
    currency: Option<String>,
) -> Result<Tracked> {
    Tracked::record(
        &domain,
        visitor,
        &session,
        &path,
        referrer.as_deref(),
        utm.unwrap_or_default(),
        event.as_deref(),
        value,
        currency.as_deref(),
    )
}

/// Record a backend event
///
/// Record a named event for a visitor on a domain your account verified.
#[tool(
    access = "accounts",
    catalog = "web.analytics-event",
    budget(written = 35000, memory = 32)
)]
pub fn event(
    domain: String,
    visitor: String,
    session: String,
    name: String,
    value: Option<i64>,
    currency: Option<String>,
) -> Result<()> {
    let account = Account::calling()?;
    drain::run(&account)?;
    track::event(
        &account,
        &domain,
        &visitor,
        &session,
        &name,
        value,
        currency.as_deref(),
    )
}

/// Measure a website funnel
///
/// Count visitors who completed two to eight page or event steps in order.
#[tool(
    access = "accounts",
    catalog = "web.analytics-funnel",
    budget(cpu = 100000, read = 1000000, written = 35000, memory = 32)
)]
pub fn funnel(
    steps: Vec<String>,
    /// Inclusive Unix timestamp in seconds.
    since: u64,
    /// Exclusive Unix timestamp in seconds. Defaults to now.
    until: Option<u64>,
) -> Result<Funnel> {
    let window = Window::new(since, until)?;
    let account = Account::calling()?;
    drain::run(&account)?;
    Funnel::report(steps, window)
}

/// Attribute visitors and Stripe revenue
///
/// Use first or last touch for the reporting window. Revenue may be behind Stripe.
#[tool(
    access = "accounts",
    catalog = "web.analytics-channels",
    budget(cpu = 100000, read = 1000000, written = 100000, memory = 32)
)]
pub fn channels(
    /// Inclusive Unix timestamp in seconds.
    since: u64,
    model: Model,
    /// Group by channel (default), source, medium, campaign, or landing page.
    by: Option<By>,
    /// Exclusive Unix timestamp in seconds. Defaults to now.
    until: Option<u64>,
) -> Result<Channels> {
    let window = Window::new(since, until)?;
    let account = Account::calling()?;
    drain::run(&account)?;
    let sync = Stripe::connected()?
        .map(|stripe| stripe.sync())
        .transpose()?;
    let mut report = Channels::report(window, model, by.unwrap_or_default())?;
    if let Some(sync) = sync {
        report.revenue_synced_through = Some(sync.synced_through);
        report.revenue_fully_synced = !sync.has_more && sync.error.is_none();
    }
    Ok(report)
}

/// Report website traffic and stored Stripe revenue
///
/// Compare the current and previous periods and break down the current period by day or week.
#[tool(
    access = "accounts",
    catalog = "web.analytics-stats",
    budget(cpu = 100000, read = 800000, written = 35000, memory = 32)
)]
pub fn stats(
    /// Inclusive Unix timestamp in seconds.
    since: u64,
    /// Exclusive Unix timestamp in seconds. Defaults to now.
    until: Option<u64>,
    /// UTC calendar day or week (starting Monday). Defaults to day.
    interval: Option<Interval>,
) -> Result<Stats> {
    let window = Window::new(since, until)?;
    let account = Account::calling()?;
    drain::run(&account)?;
    Stats::report(window, interval.unwrap_or_default())
}

/// Follow a customer or visitor's journey
///
/// Show recorded visits and stored Stripe payments in time order.
#[tool(
    access = "accounts",
    catalog = "web.analytics-journey",
    budget(cpu = 100000, read = 300000, written = 35000, memory = 32)
)]
pub fn journey(
    /// Stripe customer ID or website visitor ID.
    customer: String,
) -> Result<Journey> {
    let account = Account::calling()?;
    drain::run(&account)?;
    Journey::report(&customer)
}

/// Sync Stripe revenue
///
/// Import a bounded part of Stripe history. Call again while has_more is true.
#[tool(
    access = "accounts",
    catalog = "web.analytics-sync-revenue",
    budget(written = 100000, memory = 32)
)]
pub fn sync_revenue() -> Result<Synced> {
    let account = Account::calling()?;
    drain::run(&account)?;
    Stripe::required()?.sync()
}

/// Connect a Stripe key
///
/// Save the ID of a granted runtime secret for api.stripe.com.
#[tool(
    access = "accounts",
    catalog = "web.analytics-connect-stripe",
    budget(written = 35000, memory = 32)
)]
pub fn connect_stripe(secret_id: String) -> Result<()> {
    let account = Account::calling()?;
    drain::run(&account)?;
    Stripe::connect(&account, &secret_id)
}
