//! `crab billing` — the account's plan, credit and payments. A purchase is a
//! payment request: a code whose page the person paying opens; so is verifying
//! on GitHub, when it cannot be done from here.

use crate::{cmd::request, format, runtime, terminal};
use anyhow::{Result, bail};
use crabtalk_sdk::{
    account::{
        auth::Client as Surface,
        billing::{PortalFlow, RefillRequest},
        request::Purchase,
    },
    client::Client,
};
use serde::Serialize;

/// Where the account stands: its plan and period, credit, refill and card.
pub fn status(client: &Client, json: bool) -> Result<()> {
    let (period, credit, card) = runtime::wait(async {
        tokio::try_join!(client.period(), client.credit(), client.payment_method())
    })?;
    if json {
        #[derive(Serialize)]
        struct Status<'a, P, C, M> {
            period: &'a P,
            credit: &'a C,
            card: &'a M,
        }
        return terminal::json(&Status {
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

/// Ask a person to pay for `purchase`, of `plan` when it is one, with
/// `context` shown to them: print its page, and how to wait for it.
pub fn ask(
    client: &Client,
    purchase: Purchase,
    plan: Option<&str>,
    context: &str,
    json: bool,
) -> Result<()> {
    let asked = runtime::wait(client.ask_payment(purchase, plan, context))?;
    request::asked(&asked, json)
}

/// Refill `amount` USD when credit runs low, at most `cap` USD a month; a cap
/// of 0 turns it off.
pub fn refill(client: &Client, amount: u32, cap: u32) -> Result<()> {
    if cap > 0 && amount < 20 {
        bail!("a refill buys $20 or more");
    }
    runtime::wait(client.set_refill(&RefillRequest {
        cap_micros: i64::from(cap) * 1_000_000,
        amount_micros: i64::from(amount) * 1_000_000,
    }))?;
    match cap {
        0 => eprintln!("refill off"),
        cap => eprintln!("refill ${amount} at a time, at most ${cap} a month"),
    }
    Ok(())
}

/// Verify the account on GitHub and print when it was verified; when the check
/// fails, ask the person to verify at a code's page instead, with `context`
/// shown to them.
pub fn verify(client: &Client, context: &str, json: bool) -> Result<()> {
    let verification = runtime::wait(async {
        let verification = client.verification().await?;
        match verification.verified_at.is_none() && verification.reason.is_none() {
            true => client.verify().await,
            false => Ok(verification),
        }
    })?;
    let Some(at) = verification.verified_at else {
        let asked = runtime::wait(client.ask_verify(context))?;
        return request::asked(&asked, json);
    };
    if json {
        return terminal::json(&verification);
    }
    println!("verified {}", format::date(at));
    Ok(())
}

pub fn invoices(client: &Client, json: bool) -> Result<()> {
    let invoices = runtime::wait(client.invoices())?;
    if json {
        return terminal::json(&invoices);
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
    if !terminal::open(&runtime::wait(client.portal(Surface::Terminal, flow))?.url) {
        eprintln!("open the URL above in a browser");
    }
    Ok(())
}
