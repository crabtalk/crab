//! `crab rates` — what use past the included amounts costs.

use crate::{format, utils};
use anyhow::Result;
use crabtalk_sdk::client::Client;

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
    println!("calls         {}", per_million(rates.calls));
    println!(
        "compute       ${} per trillion instructions past each call's first {}",
        rates.overage as f64 / 1e3,
        format::count(metering.included_instructions)
    );
    println!("rows read     {}", per_million(rates.read));
    println!("rows written  {}", per_million(rates.written));
    println!("storage       ${} per GB-month", rates.stored as f64 / 1e6);
    match rates.proxied {
        Some(rate) => println!("proxied       {} bytes", per_million(rate)),
        None => println!("proxied       free"),
    }
    Ok(())
}
