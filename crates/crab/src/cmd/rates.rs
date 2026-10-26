//! `crab rates` — what use past the included amounts costs.

use crate::utils;
use anyhow::Result;
use crabtalk_sdk::{client::Client, system::meta};

/// USD micros per billion units, as USD per million.
fn per_million(rate: u64) -> String {
    format!("${} per million", rate as f64 / 1e9)
}

pub fn run(client: &Client, json: bool) -> Result<()> {
    let metering = utils::wait(client.rates())?;
    if json {
        return utils::json(&metering);
    }
    let rates = &metering.rates;
    println!("runs          {}", per_million(rates.runs));
    println!(
        "compute       ${} per million CPU-ms past each run's first {}µs",
        rates.overage as f64 / 1e6,
        metering.included_cpu
    );
    println!("rows read     {}", per_million(rates.read));
    println!("rows written  {}", per_million(rates.written));
    println!(
        "memory held   ${} per GB-hour, waiting included",
        rates.held as f64 * meta::GB_HOUR as f64 / 1e15
    );
    println!(
        "storage       ${} per GB-month past the plan's",
        rates.stored as f64 * meta::GB_MONTH as f64 / 1e15
    );
    match rates.proxied {
        Some(rate) => println!("proxied       {} bytes", per_million(rate)),
        None => println!("proxied       free"),
    }
    Ok(())
}
