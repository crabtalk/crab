//! `crab deploys` — a service's deploys on Cloud, refused and failed ones
//! included.

use crate::{format, runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::client::Client;

pub fn run(client: &Client, name: &str, json: bool) -> Result<()> {
    let status = runtime::wait(client.deployment_status(name))?;
    if json {
        return terminal::json(&status);
    }
    for approval in &status.approvals {
        println!(
            "{}  v{}  {}",
            approval.status, approval.version, approval.reference
        );
        println!(
            "  approval expires {}",
            format::instant(approval.expires_at as u64 * 1000)
        );
        println!("  {}", approval.request.url);
    }
    let deploys = status.deploys;
    if deploys.is_empty() && status.approvals.is_empty() {
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
