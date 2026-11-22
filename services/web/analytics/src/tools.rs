use crate::{
    account::Account,
    channels::{Channels, Model},
    claim::Claim,
    drain,
    funnel::Funnel,
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
    budget(written = 35000, memory = 32)
)]
pub fn funnel(
    steps: Vec<String>,
    /// Inclusive Unix timestamp in seconds.
    since: u64,
) -> Result<Funnel> {
    let account = Account::calling()?;
    drain::run(&account)?;
    Funnel::report(steps, since)
}

/// Attribute visitors and Stripe revenue by channel
///
/// Use first or last touch for the reporting window.
#[tool(
    access = "accounts",
    catalog = "web.analytics-channels",
    budget(written = 100000, memory = 32)
)]
pub fn channels(
    /// Inclusive Unix timestamp in seconds.
    since: u64,
    model: Model,
) -> Result<Channels> {
    let account = Account::calling()?;
    drain::run(&account)?;
    if let Some(stripe) = Stripe::connected()? {
        stripe.sync()?;
    }
    Channels::report(since, model)
}

/// Sync Stripe revenue
///
/// Import Stripe payments and subscription renewals using the connected key.
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
