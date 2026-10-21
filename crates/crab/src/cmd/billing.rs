//! `crab billing` — the account's plan, credit and payments. What takes a card
//! opens Stripe in the browser.

use crate::{format, utils};
use anyhow::{Result, bail};
use crabtalk_sdk::{
    account::{
        auth::Client as Surface,
        billing::{PortalFlow, RefillRequest},
    },
    client::Client,
};
use serde::Serialize;

/// Where the account stands: its plan and period, credit, refill and card.
pub fn status(client: &Client, json: bool) -> Result<()> {
    let period = utils::wait(client.period())?;
    let credit = utils::wait(client.credit())?;
    let card = utils::wait(client.payment_method())?;
    if json {
        #[derive(Serialize)]
        struct Status<'a, P, C, M> {
            period: &'a P,
            credit: &'a C,
            card: &'a M,
        }
        return utils::json(&Status {
            period: &period,
            credit: &credit,
            card: &card,
        });
    }
    println!(
        "plan    {}, {} through {}",
        period.plan,
        format::day(period.since.into()),
        format::day(i64::from(period.until) - 1)
    );
    println!("credit  {}", format::usd(credit.micros as f64));
    match credit.refill_cap_micros {
        0 => println!("refill  off"),
        cap => println!(
            "refill  {} at a time, at most {} a month",
            format::usd(credit.refill_micros as f64),
            format::usd(cap as f64)
        ),
    }
    match card {
        Some(card) => println!(
            "card    {} ending {}, expires {:02}/{}",
            card.brand, card.last4, card.exp_month, card.exp_year
        ),
        None => println!("card    none"),
    }
    println!("`crab usage` shows how much of the plan is used");
    Ok(())
}

/// Subscribe to `plan` through Checkout, or at once from a trial.
pub fn plan(client: &Client, plan: &str) -> Result<()> {
    match utils::wait(client.checkout(plan, Surface::Terminal))?.url {
        Some(url) => utils::open(&url),
        None => println!("subscribed to {plan}"),
    }
    Ok(())
}

/// Start the free trial through Checkout.
pub fn trial(client: &Client) -> Result<()> {
    utils::open(&utils::wait(client.trial(Surface::Terminal))?.url);
    Ok(())
}

/// Buy credit through Checkout, the amount chosen there.
pub fn credit(client: &Client) -> Result<()> {
    utils::open(&utils::wait(client.buy_credit(Surface::Terminal))?.url);
    Ok(())
}

/// Refill `amount` USD when credit runs low, at most `cap` USD a month; a cap
/// of 0 turns it off.
pub fn refill(client: &Client, amount: u32, cap: u32) -> Result<()> {
    if cap > 0 && amount < 20 {
        bail!("a refill buys $20 or more");
    }
    utils::wait(client.set_refill(&RefillRequest {
        cap_micros: i64::from(cap) * 1_000_000,
        amount_micros: i64::from(amount) * 1_000_000,
    }))?;
    match cap {
        0 => eprintln!("refill off"),
        cap => eprintln!("refill ${amount} at a time, at most ${cap} a month"),
    }
    Ok(())
}

pub fn invoices(client: &Client, json: bool) -> Result<()> {
    let invoices = utils::wait(client.invoices())?;
    if json {
        return utils::json(&invoices);
    }
    if invoices.is_empty() {
        println!("no invoices");
    }
    for invoice in &invoices {
        println!(
            "{}  {:<14}  {:<13}  {:>10} {}  {}",
            format::date(invoice.created),
            invoice.number.as_deref().unwrap_or("draft"),
            invoice.status,
            format::usd((invoice.amount_paid * 10_000) as f64),
            invoice.currency.to_uppercase(),
            invoice.hosted_invoice_url.as_deref().unwrap_or_default()
        );
    }
    Ok(())
}

/// Open the Stripe portal, on `flow` when one is given.
pub fn portal(client: &Client, flow: Option<PortalFlow>) -> Result<()> {
    utils::open(&utils::wait(client.portal(Surface::Terminal, flow))?.url);
    Ok(())
}
