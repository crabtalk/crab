//! `crab rm` — take programs out of service.

use crate::{project, utils};
use anyhow::{Result, bail};
use berm_build::Workspace;
use crabtalk_sdk::client::Client;
use std::path::Path;

/// Remove the programs `names`, or with none, the services of the workspace
/// the current directory selects as `crab deploy` does.
pub fn run(client: &Client, names: &[String], yes: bool) -> Result<()> {
    let names = match names.is_empty() {
        false => names.to_vec(),
        true => {
            let workspace = Workspace::read(Path::new("."), project::FILE, &[])?;
            let mut names = Vec::new();
            for dir in &workspace.services {
                names.push(project::read(dir)?.name()?.to_owned());
            }
            names
        }
    };
    if names.is_empty() {
        bail!("no service to remove here; name one");
    }
    utils::confirm(
        yes,
        &format!("remove {} and what it committed", names.join(", ")),
    )?;
    for name in &names {
        utils::wait(client.undeploy(name))?;
        eprintln!("removed {name}");
    }
    Ok(())
}
