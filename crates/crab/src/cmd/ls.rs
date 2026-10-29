//! `crab ls` — what is deployed.

use crate::{runtime, terminal};
use anyhow::Result;
use berm_api::Service;
use crabtalk_sdk::client::Client;
use serde::Serialize;

/// Enough of a digest to tell two images apart at a glance, which is all a
/// listing is for. `inspect` prints the whole one.
const SHORT: usize = 12;

/// A deployed service, and whether it is stopped.
#[derive(Serialize)]
struct Listed {
    #[serde(flatten)]
    service: Service,
    stopped: bool,
}

pub fn run(client: &Client, json: bool) -> Result<()> {
    let mut services = Vec::new();
    for service in runtime::wait(client.list())? {
        let stopped = runtime::wait(client.listing(&service.name))?.stopped;
        services.push(Listed { service, stopped });
    }
    if json {
        return terminal::json(&services);
    }
    if services.is_empty() {
        println!("no services deployed");
        return Ok(());
    }

    let width = services
        .iter()
        .map(|listed| listed.service.name.len())
        .max()
        .unwrap_or(0)
        .max("NAME".len());

    println!(
        "{:<width$}  {:<SHORT$}  {:<7}  TOOLS",
        "NAME", "DIGEST", "STATE"
    );
    for Listed { service, stopped } in &services {
        let state = match stopped {
            true => "stopped",
            false => "running",
        };
        println!(
            "{:<width$}  {:<SHORT$}  {state:<7}  {}",
            service.name,
            digest(service),
            tools(service),
        );
    }
    Ok(())
}

fn digest(service: &Service) -> &str {
    service.digest.get(..SHORT).unwrap_or(&service.digest)
}

fn tools(service: &Service) -> String {
    service
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
