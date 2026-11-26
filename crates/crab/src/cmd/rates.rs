//! `crab rates` — the deployment's rates, the same on every plan.

use crate::{runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::{
    client::Client,
    system::meta::{self, Rates},
};

/// USD micros per billion units, as USD per million.
fn per_million(rate: u64) -> String {
    format!("${} per million", rate as f64 / 1e9)
}

pub fn run(client: &Client, json: bool) -> Result<()> {
    let metering = runtime::wait(client.rates())?;
    if json {
        return terminal::json(&metering);
    }
    print(&metering.rates, metering.included_cpu, "");
    Ok(())
}

fn print(rates: &Rates, included_cpu: u64, indent: &str) {
    println!("{indent}runs          {}", per_million(rates.runs));
    println!(
        "{indent}compute       ${} per million CPU-ms past each run's first {included_cpu}µs",
        rates.overage as f64 / 1e6
    );
    println!("{indent}rows read     {}", per_million(rates.read));
    println!("{indent}rows written  {}", per_million(rates.written));
    println!(
        "{indent}memory held   ${} per million GB-s, waiting included",
        rates.held as f64 * meta::GB_SECOND as f64 / 1e9
    );
    if let Some(rotating) = rates.rotating {
        println!(
            "{indent}rotating IPs  ${} per GB, paid from credit",
            rotating as f64 / 1e6
        );
    }
}
