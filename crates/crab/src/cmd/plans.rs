//! `crab plans` — the plans the deployment sells, their limits and rates.

use crate::{cmd::rates, format, runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::{account::org::Kind, client::Client, format as display, system::meta};

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
        if plan.credit_micros > 0 {
            println!(
                "  {} credit a period",
                format::usd(plan.credit_micros as f64)
            );
        }
        if let Some(offer) = &plan.trial {
            println!(
                "  {} credit once after GitHub verification for running services, valid {} days; data kept {} days after it ends",
                format::usd(offer.amount_micros as f64),
                offer.lifetime_days,
                offer.retention_days
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
        rates::print(&plan.rates, meta::INCLUDED_CPU, "    ");
    }
    Ok(())
}
