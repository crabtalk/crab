//! `crab billing` — the account's plan, credit and payments. A purchase is a
//! payment request: a code whose page the person paying opens.

use crate::{format, utils};
use anyhow::{Result, bail};
use crabtalk_sdk::{
    account::{
        auth::Client as Surface,
        billing::{
            CreateRequest, PaymentRequest, PortalFlow, Purchase, RefillRequest, RequestStatus,
        },
    },
    client::Client,
};
use serde::Serialize;
use std::{
    thread,
    time::{Duration, Instant},
};

/// How often `--wait` asks after a request.
const POLL: Duration = Duration::from_secs(2);

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

/// Ask a person to pay for `purchase`, of `plan` when it is one, with
/// `context` shown to them: print its page, and how to wait for it.
pub fn request(
    client: &Client,
    purchase: Purchase,
    plan: Option<&str>,
    context: &str,
    json: bool,
) -> Result<()> {
    let request = utils::wait(client.request_payment(&CreateRequest {
        purchase,
        plan: plan.map(str::to_owned),
        context: context.to_owned(),
    }))?;
    let wait = format!("crab billing request {} --wait", request.code);
    if json {
        #[derive(Serialize)]
        struct Next<'a> {
            command: &'a str,
            until: &'a str,
        }
        #[derive(Serialize)]
        struct Asked<'a> {
            #[serde(flatten)]
            request: &'a PaymentRequest,
            next: Next<'a>,
        }
        return utils::json(&Asked {
            request: &request,
            next: Next {
                command: &wait,
                until: "status is no longer pending",
            },
        });
    }
    let pay = request
        .url
        .trim_end_matches(&request.code)
        .trim_end_matches('/');
    println!("{}", request.url);
    eprintln!(
        "{}: open the link, or enter {} at {pay}, within {} minutes",
        describe(&request),
        request.code,
        (request.expires_at - request.created_at) / 60
    );
    eprintln!("then `{wait}`");
    Ok(())
}

/// What `request` buys, priced.
fn describe(request: &PaymentRequest) -> String {
    let plan = request.plan.as_deref().unwrap_or_default();
    let monthly = request
        .price_micros
        .map(|micros| format!("{} a month", format::usd(micros as f64)))
        .unwrap_or_default();
    match request.purchase {
        Purchase::Plan if request.card_on_file => {
            format!("{plan} at {monthly}, ending the trial and charging its card at once")
        }
        Purchase::Plan => format!("{plan} at {monthly}"),
        Purchase::Trial => format!(
            "a {plan} trial, free for {} days, then {monthly}",
            request.trial_days.unwrap_or_default()
        ),
        Purchase::Credit => "credit, the amount chosen at checkout".to_owned(),
    }
}

/// Where payment request `code` stands. With `wait`, once it is no longer
/// pending, giving up after `timeout` seconds. Fails unless it completed or,
/// without `wait`, is still pending.
pub fn status_of(client: &Client, code: &str, wait: bool, timeout: u64, json: bool) -> Result<()> {
    let started = Instant::now();
    let request = loop {
        let request = utils::wait(client.payment_request(code))?;
        let waiting = request.status == RequestStatus::Pending
            && started.elapsed() < Duration::from_secs(timeout);
        if !wait || !waiting {
            break request;
        }
        thread::sleep(POLL);
    };
    if json {
        utils::json(&request)?;
    } else {
        println!(
            "{}  {}  {}",
            request.code,
            request.status,
            describe(&request)
        );
    }
    match request.status {
        RequestStatus::Completed => Ok(()),
        RequestStatus::Pending if wait => bail!("still pending after {timeout}s"),
        RequestStatus::Pending => Ok(()),
        RequestStatus::Declined => bail!("the person declined it"),
        RequestStatus::Expired => bail!("it expired; ask again for a new code"),
    }
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
