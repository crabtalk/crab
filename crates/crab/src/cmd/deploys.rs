//! `crab deploys` — a service's deploys on Cloud, refused and failed ones
//! included.

use crate::{format, utils};
use anyhow::Result;
use crabtalk_sdk::client::Client;

pub fn run(client: &Client, name: &str, json: bool) -> Result<()> {
    let deploys = utils::wait(client.deploys(name))?;
    if json {
        return utils::json(&deploys);
    }
    if deploys.is_empty() {
        println!("no deploys recorded");
    }
    for deploy in &deploys {
        let version = deploy
            .version
            .as_deref()
            .map_or("-".to_owned(), |v| format!("v{v}"));
        println!(
            "{:<9} {:<12} {}  by {}  {}",
            deploy.outcome,
            version,
            format::instant(deploy.at as u64 * 1000),
            deploy.by,
            deploy.reference
        );
        if !deploy.migrations.is_empty() {
            println!("  migrated {}", deploy.migrations.join(", "));
        }
        if deploy.seeded {
            println!("  seeded");
        }
        if let Some(reason) = &deploy.reason {
            println!("  {reason}");
        }
    }
    Ok(())
}
