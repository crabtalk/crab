//! `crab cost` — what a service cost in a month, by resource.

use crate::{format, utils};
use anyhow::Result;
use crabtalk_sdk::client::Client;

pub fn run(client: &Client, name: &str, month: Option<&str>, json: bool) -> Result<()> {
    let billing = utils::wait(client.billing(name, month))?;
    if json {
        return utils::json(&billing);
    }
    let total: i64 = billing.days.iter().map(|day| day.cost_micros).sum();
    println!(
        "{name} in {}: {} at the rates, before the account's included amounts",
        billing.month,
        format::usd(total as f64)
    );
    println!("{:<12}  {:>16}  {:>12}", "RESOURCE", "UNITS", "COST");
    for resource in &billing.resources {
        let cost = resource.cost_micros_decimal.parse::<f64>().unwrap_or(0.0);
        println!(
            "{:<12}  {:>16}  {:>12}",
            resource.resource,
            format::count(resource.units),
            format::usd(cost)
        );
    }
    if billing.owed_micros > 0 {
        println!(
            "the account owes {} past its credit",
            format::usd(billing.owed_micros as f64)
        );
    }
    Ok(())
}
