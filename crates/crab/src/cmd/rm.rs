//! `crab rm` — take programs out of service.

use crate::{project, runtime, terminal};
use anyhow::Result;
use crabtalk_sdk::client::Client;

/// Remove the programs `names`, or with none, the services the current
/// directory selects.
pub fn run(client: &Client, names: &[String], yes: bool) -> Result<()> {
    let names = project::selected(names)?;
    terminal::confirm(
        yes,
        &format!("remove {} and what it committed", names.join(", ")),
    )?;
    for name in &names {
        runtime::wait(client.undeploy(name))?;
        eprintln!("removed {name}");
    }
    Ok(())
}
