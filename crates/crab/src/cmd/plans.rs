//! `crab plans` — the plans the deployment sells and their limits.

use crate::{format, runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::{account::org::Kind, client::Client, format as display};

pub fn run(client: &Client, json: bool) -> Result<()> {
    let plans = runtime::wait(client.plans())?;
    if json {
        return terminal::json(&plans);
    }
    for plan in &plans {
        let price = plan.price_micros.map_or_else(
            || "not sold".to_owned(),
            |micros| format!("{}/month", format::usd(micros as f64)),
        );
        let audience = plan
            .accounts
            .iter()
            .map(|kind| match kind {
                Kind::User => "individuals",
                Kind::Org => "organizations",
            })
            .collect::<Vec<_>>()
            .join(" and ");
        println!("{}  {price}  for {audience}", plan.slug);
        if plan.usage_micros > 0 {
            println!(
                "  {} of compute included a period, at the rates `crab rates` shows",
                format::usd(plan.usage_micros as f64)
            );
        }
        println!(
            "  {} storage, {} outbound data a period",
            display::bytes(plan.included.stored),
            display::bytes(plan.included.sent)
        );
        match plan.memory {
            0 => println!("  no limit on memory held"),
            memory => println!("  {} of memory held at once", display::bytes(memory)),
        }
    }
    Ok(())
}
