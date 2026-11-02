//! `crab cron` — a service's cron tools: listed.

use crate::{format, runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::client::Client;

pub fn ls(client: &Client, name: &str, json: bool) -> Result<()> {
    let crons = runtime::wait(client.crons(name))?;
    if json {
        return terminal::json(&crons);
    }
    if crons.held {
        println!("held: its account is refused, so nothing fires");
    }
    for job in &crons.crons {
        let next = job.due.map_or_else(|| "-".to_owned(), format::instant);
        println!(
            "{:<24}  {:<16}  next {next}",
            job.tool,
            job.cron.as_deref().unwrap_or("stopped")
        );
    }
    Ok(())
}
