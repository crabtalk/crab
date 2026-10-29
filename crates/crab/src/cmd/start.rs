//! `crab start` — serve stopped programs again.

use crate::{project, runtime};
use anyhow::Result;
use crabtalk_sdk::client::Client;

/// Start the programs `names`, or with none, the services the current
/// directory selects.
pub fn run(client: &Client, names: &[String]) -> Result<()> {
    for name in &project::selected(names)? {
        runtime::wait(client.start(name))?;
        eprintln!("started {name}");
    }
    Ok(())
}
