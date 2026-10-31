//! `crab stop` — stop serving services, keeping what they committed.

use crate::{project, runtime};
use anyhow::Result;
use crabtalk_sdk::client::Client;

/// Stop the services `names`, or with none, the services the current
/// directory selects.
pub fn run(client: &Client, names: &[String]) -> Result<()> {
    for name in &project::selected(names)? {
        runtime::wait(client.stop(name))?;
        eprintln!("stopped {name}");
    }
    Ok(())
}
