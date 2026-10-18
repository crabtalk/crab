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
        if included.calls
            + included.instructions
            + included.read
            + included.written
            + included.stored
            == 0
        {
            println!("  includes no amounts");
            continue;
        }
        println!(
            "  {} calls, {} instructions, {} rows read, {} rows written, {}",
            format::count(included.calls),
            format::count(included.instructions),
            format::count(included.read),
            format::count(included.written),
            format::bytes(included.stored)
        );
        match plan.concurrency {
            0 => println!("  no cap on calls at once"),
            cap => println!("  {cap} calls at once"),
        }
    }
    Ok(())
}
