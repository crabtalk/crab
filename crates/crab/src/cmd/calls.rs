//! `crab calls` — a service's calls over the last 30 days, newest first.

use crate::{format, utils};
use anyhow::Result;
use crabtalk_sdk::client::Client;

pub fn run(
    client: &Client,
    name: &str,
    tool: Option<&str>,
    outcome: Option<&str>,
    limit: u32,
    json: bool,
) -> Result<()> {
    let calls = utils::wait(client.calls(name, tool, outcome, limit))?;
    if json {
        return utils::json(&calls);
    }
    if calls.is_empty() {
        println!("no calls in the last 30 days");
        return Ok(());
    }
    println!(
        "{:<20}  {:<20}  {:<8}  {:>8}  {:<28}  COST",
        "AT", "TOOL", "OUTCOME", "MS", "CALLER"
    );
    for costed in &calls {
        let call = &costed.call;
        let cost = costed
            .cost_micros_decimal
            .as_deref()
            .and_then(|micros| micros.parse::<f64>().ok())
            .map_or_else(|| "-".to_owned(), format::usd);
        println!(
            "{:<20}  {:<20}  {:<8}  {:>8}  {:<28}  {cost}",
            format::instant(call.at),
            call.tool,
            call.outcome,
            call.ms,
            format!("{} {}", call.source, call.caller),
        );
    }
    Ok(())
}
