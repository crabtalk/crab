//! `crab rates` — deployment and plan rates.

use crate::{runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::{
    client::Client,
    system::meta::{self, Metering, Rates},
};
use serde::Serialize;
use std::collections::BTreeMap;

/// USD micros per billion units, as USD per million.
fn per_million(rate: u64) -> String {
    format!("${} per million", rate as f64 / 1e9)
}

pub fn run(client: &Client, json: bool) -> Result<()> {
    let (metering, plans) = runtime::wait(async {
        if client.owner.is_some() {
            tokio::try_join!(client.rates(), client.plans())
        } else {
            Ok((client.rates().await?, Vec::new()))
        }
    })?;
    if json {
        #[derive(Serialize)]
        struct Pricing<'a> {
            #[serde(flatten)]
            metering: &'a Metering,
            #[serde(skip_serializing_if = "BTreeMap::is_empty")]
            plans: BTreeMap<&'a str, &'a Rates>,
        }
        return terminal::json(&Pricing {
            metering: &metering,
            plans: plans
                .iter()
                .map(|plan| (plan.slug.as_str(), &plan.rates))
                .collect(),
        });
    }
    if !plans.is_empty() {
        println!("list");
    }
    print(&metering.rates, metering.included_cpu, "");
    for plan in &plans {
        println!("\n{}", plan.slug);
        print(&plan.rates, metering.included_cpu, "");
    }
    Ok(())
}

pub(super) fn print(rates: &Rates, included_cpu: u64, indent: &str) {
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
            "{indent}rotating IPs  ${} per GB, paid from purchased credit",
            rotating as f64 / 1e6
        );
    }
}
