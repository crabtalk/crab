//! `crab stop` — stop serving programs, keeping what they committed.

use crate::{project, utils};
use anyhow::Result;
use crabtalk_sdk::client::Client;

/// Stop the programs `names`, or with none, the services the current
/// directory selects.
pub fn run(client: &Client, names: &[String]) -> Result<()> {
    for name in &project::selected(names)? {
        utils::wait(client.stop(name))?;
        eprintln!("stopped {name}");
    }
    Ok(())
}
