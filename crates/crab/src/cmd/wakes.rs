//! `crab wakes` — the wakes armed to call a service's tools.

use crate::{format, utils};
use anyhow::Result;
use crabtalk_sdk::client::Client;

pub fn run(client: &Client, name: &str, json: bool) -> Result<()> {
    let wakes = utils::wait(client.wakes(name))?;
    if json {
        return utils::json(&wakes);
    }
    if wakes.held {
        println!("held: its account is refused, so nothing fires");
    }
    if wakes.wakes.is_empty() {
        println!("no wakes armed");
    }
    for wake in &wakes.wakes {
        println!(
            "{:<24}  due {}  by {}  {} failures",
            wake.tool,
            format::instant(wake.due),
            wake.by,
            wake.failures
        );
    }
    Ok(())
}
