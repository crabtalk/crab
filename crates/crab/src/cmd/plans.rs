//! `crab plans` — the plans the deployment sells, and what each includes.

use crate::{format, utils};
use anyhow::Result;
use crabtalk_sdk::client::Client;

pub fn run(client: &Client, json: bool) -> Result<()> {
    let plans = utils::wait(client.plans())?;
    if json {
        return utils::json(&plans);
    }
    for plan in &plans {
        let price = plan.price_micros.map_or_else(
            || "not sold".to_owned(),
            |micros| format!("{}/month", format::usd(micros as f64)),
        );
        let kind = plan
            .kind
            .map_or("anyone".to_owned(), |kind| kind.to_string());
        println!("{}  {price}  for {kind}", plan.slug);
        let included = &plan.included;
        if included.runs
            + included.cpu
            + included.read
            + included.written
            + included.held
            + included.stored
            == 0
        {
            println!("  includes no amounts");
            continue;
        }
        println!(
            "  {} runs, {} CPU µs, {} rows read, {} rows written, {} memory held, {}",
            format::count(included.runs),
            format::count(included.cpu),
            format::count(included.read),
            format::count(included.written),
            format::gb_hours(included.held),
            format::bytes(included.stored)
        );
        match plan.memory {
            0 => println!("  no limit on memory held"),
            memory => println!("  {} of memory held at once", format::bytes(memory)),
        }
    }
    Ok(())
}
