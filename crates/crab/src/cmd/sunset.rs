//! `crab sunset` — when a deprecated tool stops being served.

use crate::runtime;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use crabtalk_sdk::client::Client;

/// Stop serving `name`'s `tool` from UTC day `on`, `YYYY-MM-DD`; `None` clears
/// it.
pub fn run(client: &Client, name: &str, tool: &str, on: Option<&str>) -> Result<()> {
    let sunset = on
        .map(|on| {
            NaiveDate::parse_from_str(on, "%Y-%m-%d")
                .with_context(|| format!("{on:?} is not YYYY-MM-DD"))
                .map(|day| {
                    day.and_hms_opt(0, 0, 0)
                        .expect("midnight")
                        .and_utc()
                        .timestamp()
                })
        })
        .transpose()?;
    runtime::wait(client.sunset(name, tool, sunset))?;
    match on {
        Some(on) => eprintln!("{name}.{tool} stops being served on {on}"),
        None => eprintln!("{name}.{tool} has no sunset"),
    }
    Ok(())
}
