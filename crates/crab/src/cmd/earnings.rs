//! `crab earnings` — what the account's priced tools earned, a UTC calendar
//! month each, what is owed and the payouts; `connect` and `dashboard` open
//! the account's Stripe Connect pages.

use crate::{format, runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::{account::billing::Earned, client::Client};

pub fn run(client: &Client, json: bool) -> Result<()> {
    let earnings = runtime::wait(client.earnings())?;
    if json {
        return terminal::json(&earnings);
    }
    if earnings.months.is_empty() {
        println!("no priced calls yet");
    } else {
        let width = earnings
            .months
            .iter()
            .flat_map(|month| &month.tools)
            .map(|tool| tool.service.len() + tool.tool.len() + 3)
            .max()
            .unwrap_or(0)
            .max(7);
        println!(
            "{:<width$}  {:>10}  {:>10}  {:>10}  {:>10}  {:>10}",
            "MONTH", "CALLS", "REVENUE", "CUT", "COMPUTE", "NET"
        );
        for month in &earnings.months {
            line(&month.month, &month.earned, width);
            for tool in &month.tools {
                line(
                    &format!("  {}.{}", tool.service, tool.tool),
                    &tool.earned,
                    width,
                );
            }
        }
        println!("compute is every priced call at the rates, failed ones included");
    }
    println!();
    println!("owed     {}", format::usd(earnings.owed as f64));
    match (earnings.connected, earnings.payouts_enabled) {
        (false, _) => println!("payouts  no payout account: `crab earnings connect`"),
        (true, false) => {
            println!("payouts  not enabled yet: finish onboarding with `crab earnings connect`")
        }
        (true, true) => println!("payouts  enabled"),
    }
    if !earnings.payouts.is_empty() {
        println!();
        println!(
            "{:<7}  {:>10}  {:>10}  {:<7}  SENT",
            "MONTH", "AMOUNT", "FEES", "STATUS"
        );
        for payout in &earnings.payouts {
            println!(
                "{:<7}  {:>10}  {:>10}  {:<7}  {}",
                payout.month,
                format::usd(payout.amount as f64),
                format::usd(payout.fees as f64),
                payout.status,
                format::date(payout.created_at),
            );
        }
    }
    Ok(())
}

/// Open Stripe's onboarding for the account's payout account.
pub fn connect(client: &Client) -> Result<()> {
    if !terminal::open(&runtime::wait(client.earnings_connect())?.url) {
        eprintln!("open the URL above in a browser");
    }
    Ok(())
}

/// Open the payout account's Stripe Express dashboard.
pub fn dashboard(client: &Client) -> Result<()> {
    if !terminal::open(&runtime::wait(client.earnings_dashboard())?.url) {
        eprintln!("open the URL above in a browser");
    }
    Ok(())
}

fn line(label: &str, earned: &Earned, width: usize) {
    println!(
        "{label:<width$}  {:>10}  {:>10}  {:>10}  {:>10}  {:>10}",
        format::count(earned.calls),
        format::usd(earned.revenue as f64),
        format::usd(earned.cut as f64),
        format::usd(earned.compute as f64),
        format::usd(earned.net as f64),
    );
}
